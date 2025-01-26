use anyhow::Context as _;
use itertools::Itertools as _;
use std::net::{Ipv4Addr, Ipv6Addr};

pub fn interface_ipv4(iface: &str) -> anyhow::Result<Ipv4Addr> {
    let addrs: Vec<Ipv4Addr> = nix::ifaddrs::getifaddrs()
        .context("Failed to obtain network interface information")?
        .filter(|i| i.interface_name == iface)
        .filter_map(|ifaddr| ifaddr.address)
        .filter_map(|sockaddr| sockaddr.as_sockaddr_in().map(|sockaddr4| sockaddr4.ip()))
        .filter(|ip| ip.is_global())
        .unique()
        .collect();

    if addrs.len() > 1 {
        log::warn!("Multiple global IPv4 addresses found on interface '{iface}'")
    }

    addrs
        .first()
        .copied()
        .with_context(|| format!("Interface '{iface}' does not have a global IPv4 address"))
}

const PREFIX_MASK: Ipv6Addr = Ipv6Addr::new(0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0, 0, 0, 0);

pub fn interface_ipv6_prefix(iface: &str) -> anyhow::Result<Ipv6Addr> {
    let addrs: Vec<Ipv6Addr> = nix::ifaddrs::getifaddrs()
        .context("Failed to obtain network interface information")?
        .filter(|i| i.interface_name == iface)
        .filter_map(|ifaddr| ifaddr.address)
        .filter_map(|sockaddr| sockaddr.as_sockaddr_in6().map(|sockaddr6| sockaddr6.ip()))
        .filter(|ip| ip.is_unicast_global())
        .map(|ip| ip & PREFIX_MASK)
        .unique()
        .collect();

    if addrs.len() > 1 {
        log::warn!("Multiple unicast global IPv6 prefixes found on interface '{iface}'")
    }

    addrs.first().copied().with_context(|| {
        format!("Interface '{iface}' does not have an unicast global IPv6 address")
    })
}

pub async fn http_get_ipv4(url: url::Url) -> anyhow::Result<Ipv4Addr> {
    let ip: Ipv4Addr = reqwest::get(url.clone())
        .await
        .with_context(|| format!("Failed to GET {url}"))?
        .text()
        .await?
        .trim()
        .parse::<Ipv4Addr>()
        .with_context(|| format!("Unexptected data from {url}"))?;
    Ok(ip)
}

pub async fn http_get_ipv6_prefix(url: url::Url) -> anyhow::Result<Ipv6Addr> {
    let ip: Ipv6Addr = reqwest::get(url.clone())
        .await
        .with_context(|| format!("Failed to GET {url}"))?
        .text()
        .await?
        .trim()
        .parse::<Ipv6Addr>()
        .with_context(|| format!("Unexptected data from {url}"))?;
    Ok(ip & PREFIX_MASK)
}
