use serde::{Deserialize, Serialize};
use std::fmt::{self, Write};
use std::net::{Ipv4Addr, Ipv6Addr};

#[derive(Deserialize, Debug)]
pub struct Zone {
    pub id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename = "status", rename_all = "lowercase")]
pub enum Status {
    Active,
    Pending,
    Initializing,
    Moved,
    Deleted,
    Deactivated,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "type")]
#[allow(clippy::upper_case_acronyms)]
pub enum DnsContent {
    A { content: Ipv4Addr },
    AAAA { content: Ipv6Addr },
    CNAME { content: String },
    NS { content: String },
    MX { content: String, priority: u16 },
    TXT { content: String },
    SRV { content: String },
}

#[derive(Deserialize, Debug)]
pub struct DnsRecord {
    pub name: String,
    #[serde(flatten)]
    pub content: DnsContent,
    pub id: String,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct ListZonesParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_page: Option<u32>,
    #[serde(rename = "match", skip_serializing_if = "Option::is_none")]
    pub search_match: Option<SearchMatch>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum SearchMatch {
    All,
    Any,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct ListDnsRecordsParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_page: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<OrderDirection>,
}

#[derive(Serialize, Clone, Debug)]
#[allow(dead_code)]
pub enum OrderDirection {
    Asc,
    Desc,
}

#[derive(Serialize, Clone, Debug)]
pub struct UpdateDnsRecordParams<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxied: Option<bool>,
    pub name: &'a str,
    #[serde(flatten)]
    pub content: DnsContent,
}

#[derive(Deserialize, Debug, Eq, PartialEq)]
pub struct ResultInfo {
    pub page: u32,
    pub per_page: u32,
    pub total_pages: u32,
    pub count: u32,
    pub total_count: u32,
}

#[derive(Deserialize, Debug, Eq, PartialEq)]
pub struct ApiResponse<T> {
    pub result: T,
    pub result_info: Option<ResultInfo>,
    #[serde(default)]
    pub messages: Vec<ApiMessage>,
    #[serde(default)]
    pub errors: Vec<ApiMessage>,
}

#[derive(Deserialize, Debug, Eq, PartialEq)]
pub struct ApiMessage {
    pub code: u16,
    pub message: String,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug)]
pub enum ApiError {
    Http(reqwest::StatusCode, Vec<ApiMessage>),
    Request(reqwest::Error),
}

impl std::error::Error for ApiError {}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::Http(status, errors) => {
                let mut out = format!("HTTP {status}");
                for err in errors {
                    let _ = write!(out, "\n{}: {}", err.code, err.message);
                }
                write!(f, "{out}")
            }
            ApiError::Request(e) => write!(f, "{e}"),
        }
    }
}

impl From<reqwest::Error> for ApiError {
    fn from(e: reqwest::Error) -> Self {
        ApiError::Request(e)
    }
}

const API_BASE: &str = "https://api.cloudflare.com/client/v4/";

pub struct Client {
    http: reqwest::Client,
    auth_header: String,
}

impl Client {
    pub fn new(token: String) -> Result<Client, reqwest::Error> {
        Ok(Client {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()?,
            auth_header: format!("Bearer {token}"),
        })
    }

    pub async fn list_zones(
        &self,
        params: &ListZonesParams,
    ) -> Result<ApiResponse<Vec<Zone>>, ApiError> {
        let resp = self
            .http
            .get(format!("{API_BASE}zones"))
            .header("Authorization", &self.auth_header)
            .query(params)
            .send()
            .await?;

        self.handle_response(resp).await
    }

    pub async fn list_dns_records(
        &self,
        zone_id: &str,
        params: &ListDnsRecordsParams,
    ) -> Result<ApiResponse<Vec<DnsRecord>>, ApiError> {
        let resp = self
            .http
            .get(format!("{API_BASE}zones/{zone_id}/dns_records"))
            .header("Authorization", &self.auth_header)
            .query(params)
            .send()
            .await?;

        self.handle_response(resp).await
    }

    pub async fn update_dns_record(
        &self,
        zone_id: &str,
        record_id: &str,
        params: &UpdateDnsRecordParams<'_>,
    ) -> Result<ApiResponse<DnsRecord>, ApiError> {
        let resp = self
            .http
            .put(format!("{API_BASE}zones/{zone_id}/dns_records/{record_id}"))
            .header("Authorization", &self.auth_header)
            .json(params)
            .send()
            .await?;

        self.handle_response(resp).await
    }

    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        resp: reqwest::Response,
    ) -> Result<ApiResponse<T>, ApiError> {
        let status = resp.status();
        if status.is_success() {
            resp.json::<ApiResponse<T>>()
                .await
                .map_err(ApiError::Request)
        } else {
            let errors: Vec<ApiMessage> = resp
                .json::<serde_json::Value>()
                .await
                .ok()
                .and_then(|v| {
                    v.get("errors")
                        .and_then(|e| serde_json::from_value(e.clone()).ok())
                })
                .unwrap_or_default();
            Err(ApiError::Http(status, errors))
        }
    }
}
