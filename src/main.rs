#![feature(ip)]

mod config;
mod ip;

use anyhow::Context as _;
use cloudflare::{
    endpoints::{dns, zone},
    framework::{async_api::Client, SearchMatch},
};
use config::{save_history, Config, History, ZoneConfig};
use hashbrown::HashMap;
use ip::{http_get_ipv4, http_get_ipv6_prefix, interface_ipv4, interface_ipv6_prefix};
use serde::Deserialize;
use std::net::{Ipv4Addr, Ipv6Addr};

#[derive(Deserialize)]
#[allow(dead_code)]
struct PageInfo {
    count: u32,
    page: u32,
    per_page: u32,
    total_count: u32,
    total_pages: u32,
}

/// Given a zone name return the zone identifier.
async fn zone_id(name: &str, api_client: &Client) -> anyhow::Result<String> {
    let endpoint = zone::ListZones {
        params: zone::ListZonesParams {
            name: Some(name.to_string()),
            status: Some(zone::Status::Active),
            search_match: Some(SearchMatch::All),
            ..Default::default()
        },
    };

    let response = api_client
        .request(&endpoint)
        .await
        .context("Failed to list zones")?;

    if response.result.len() > 1 {
        anyhow::bail!("Multiple zones matching name {name}");
    }

    let id: String = response
        .result
        .first()
        .with_context(|| format!("No zones matching name {name}"))?
        .id
        .clone();

    Ok(id)
}

struct RecordMaps {
    a: HashMap<String, String>,
    aaaa: HashMap<String, String>,
}

async fn zone_record_map(zone_identifier: &str, api_client: &Client) -> anyhow::Result<RecordMaps> {
    let mut a_record_id_map: HashMap<String, String> = HashMap::new();
    let mut aaaa_record_id_map: HashMap<String, String> = HashMap::new();

    let mut page: u32 = 1;
    loop {
        let endpoint = dns::ListDnsRecords {
            zone_identifier,
            params: dns::ListDnsRecordsParams {
                direction: Some(cloudflare::framework::OrderDirection::Ascending),
                page: Some(page),
                ..Default::default()
            },
        };

        let response = api_client
            .request(&endpoint)
            .await
            .context("Failed to list existing DNS records")?;

        let a_record_id_map_per_page: HashMap<String, String> = response
            .result
            .iter()
            .filter(|record| matches!(record.content, dns::DnsContent::A { content: _ }))
            .map(|record| (record.name.clone(), record.id.clone()))
            .collect();
        a_record_id_map.extend(a_record_id_map_per_page);

        let aaaa_record_id_map_per_page: HashMap<String, String> = response
            .result
            .iter()
            .filter(|record| matches!(record.content, dns::DnsContent::AAAA { content: _ }))
            .map(|record| (record.name.clone(), record.id.clone()))
            .collect();
        aaaa_record_id_map.extend(aaaa_record_id_map_per_page);

        if let Some(result_info) = response.result_info {
            let page_info: PageInfo = serde_json::from_value(result_info)
                .context("Unexpected response from Cloudflare's list DNS records API")?;

            if page_info.total_pages == page {
                break;
            }

            page = page.checked_add(1).context("Page number wrapped")?;
        } else {
            break;
        }
    }

    Ok(RecordMaps {
        a: a_record_id_map,
        aaaa: aaaa_record_id_map,
    })
}

async fn update_zone(
    api_client: &Client,
    config: &ZoneConfig,
    ipv4: Option<Ipv4Addr>,
    ipv6_prefix: Option<Ipv6Addr>,
) -> anyhow::Result<()> {
    let zone_name: &str = config.name.as_str();

    if config.records.is_empty() {
        log::warn!("No records for zone '{zone_name}'");
        return Ok(());
    }

    let zone_identifier = zone_id(zone_name, api_client)
        .await
        .with_context(|| format!("Failed to get zone identifer from zone name '{zone_name}'"))?;

    let record_maps: RecordMaps = zone_record_map(zone_identifier.as_str(), api_client)
        .await
        .with_context(|| {
            format!("Failed to list records for zone '{zone_name}' id '{zone_identifier}'")
        })?;

    let mut records_to_update: Vec<dns::UpdateDnsRecord> = Vec::with_capacity(config.records.len());

    let mut errors: u32 = 0;

    for record_config in &config.records {
        let record_name: &str = record_config.name.as_str();

        if let Some(content) = ipv4 {
            if let Some(record_id) = record_maps.a.get(record_name) {
                log::debug!("Update {record_name} A to {content}");

                records_to_update.push(dns::UpdateDnsRecord {
                    zone_identifier: zone_identifier.as_str(),
                    identifier: record_id.as_str(),
                    params: dns::UpdateDnsRecordParams {
                        ttl: record_config.ttl,
                        proxied: record_config.proxied,
                        name: record_config.name.as_str(),
                        content: dns::DnsContent::A { content },
                    },
                });
            } else {
                log::error!("No A record exists for {record_name}");
                errors = errors.saturating_add(1);
            }
        }

        if let (Some(prefix), Some(suffix)) = (ipv6_prefix, &record_config.suffix) {
            if let Some(record_id) = record_maps.aaaa.get(record_name) {
                let content: Ipv6Addr = prefix | suffix;

                log::debug!("Update {record_name} AAAA to {content}");

                records_to_update.push(dns::UpdateDnsRecord {
                    zone_identifier: zone_identifier.as_str(),
                    identifier: record_id.as_str(),
                    params: dns::UpdateDnsRecordParams {
                        ttl: record_config.ttl,
                        proxied: record_config.proxied,
                        name: record_config.name.as_str(),
                        content: dns::DnsContent::AAAA { content },
                    },
                });
            } else {
                log::error!("No AAAA record exists for {record_name}");
                errors = errors.saturating_add(1);
            }
        }
    }

    let requests: Vec<_> = records_to_update
        .iter()
        .map(|endpoint| api_client.request(endpoint))
        .collect();

    let results: Vec<_> = futures::future::join_all(requests).await;

    for result in results {
        if let Err(e) = result {
            log::error!("Failed to update record for zone '{zone_name}': {e:?}");
            errors = errors.saturating_add(1);
        }
    }

    if errors > 0 {
        anyhow::bail!("Failed to update {errors} records");
    }

    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let config: Config = Config::from_args_os()?;

    if config.zones.is_empty() {
        log::warn!("No zones specified in configuration");
        return Ok(());
    }

    let ipv4: Option<Ipv4Addr> = {
        if let Some(iface) = config.a_interface {
            Some(interface_ipv4(&iface)?)
        } else if let Some(url) = config.a_http {
            Some(http_get_ipv4(url).await?)
        } else {
            None
        }
    };

    let ipv6_prefix: Option<Ipv6Addr> = {
        if let Some(iface) = config.aaaa_interface {
            Some(interface_ipv6_prefix(&iface)?)
        } else if let Some(url) = config.aaaa_http {
            Some(http_get_ipv6_prefix(url).await?)
        } else {
            None
        }
    };

    if ipv4.is_none() && ipv6_prefix.is_none() {
        log::warn!("Both IPv4 and IPv6 disabled in configuration");
        return Ok(());
    }

    let new_ipv4: Option<Ipv4Addr> = match (ipv4, config.history.ipv4) {
        (None, _) => None,
        (Some(ip), None) => {
            log::warn!("Previous IPv4 unknown, updating to {ip}");
            Some(ip)
        }
        (Some(ip), Some(prev)) => {
            if ip == prev {
                log::info!("IPv4 unchanged, skipping update");
                None
            } else {
                log::warn!("IPv4 changed from {prev} to {ip}");
                Some(ip)
            }
        }
    };

    let new_ipv6_prefix: Option<Ipv6Addr> = match (ipv6_prefix, config.history.ipv6_prefix) {
        (None, _) => None,
        (Some(prefix), None) => {
            log::warn!("Previous IPv6 prefix unknown, updating to {prefix}");
            Some(prefix)
        }
        (Some(prefix), Some(prev)) => {
            if prefix == prev {
                log::info!("IPv6 prefix unchanged, skipping update");
                None
            } else {
                log::warn!("IPv6 prefix changed from {prev} to {prefix}");
                Some(prefix)
            }
        }
    };

    if new_ipv4.is_none() && new_ipv6_prefix.is_none() {
        return Ok(());
    }

    let zone_updates: Vec<_> = config
        .zones
        .iter()
        .map(|zone| update_zone(&config.cloudflare_client, zone, new_ipv4, new_ipv6_prefix))
        .collect();

    let results: Vec<anyhow::Result<()>> = futures::future::join_all(zone_updates).await;

    let mut errors: u32 = 0;
    for result in results {
        if let Err(e) = result {
            log::error!("Failed to update zone: {e:?}");
            errors = errors.saturating_add(1);
        }
    }

    if errors > 0 {
        anyhow::bail!("Failed to update {errors} zones");
    }

    save_history(
        &config.history_path,
        History {
            ipv4: new_ipv4,
            ipv6_prefix: new_ipv6_prefix,
        },
    )
    .context("Failed to save history")
}
