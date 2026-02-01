//! Low-level Cloudflare DNS API client.
//!
//! This module provides direct access to the Cloudflare DNS API.
//!
//! # API Reference
//!
//! - [DNS Records](https://developers.cloudflare.com/api/resources/dns/subresources/records/)
//! - [Zones](https://developers.cloudflare.com/api/resources/zones/)

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::HttpClientConfig;

/// The Cloudflare API base URL.
const CLOUDFLARE_API_URL: &str = "https://api.cloudflare.com/client/v4";

/// Errors that may occur when interacting with the Cloudflare API.
#[derive(Debug, Error)]
pub enum CloudflareError {
    /// The API returned an error response.
    #[error("API error: {0}")]
    Api(ApiError),

    /// An HTTP request error occurred.
    #[error("HTTP request error: {0}")]
    Request(#[from] reqwest::Error),

    /// Failed to serialize/deserialize.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Cloudflare API error.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiError {
    /// Error code.
    pub code: i32,
    /// Error message.
    pub message: String,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

/// Cloudflare API response wrapper.
#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    success: bool,
    errors: Vec<ApiError>,
    #[serde(default)]
    #[allow(dead_code)]
    messages: Vec<serde_json::Value>,
    result: Option<T>,
    result_info: Option<ResultInfo>,
}

/// Pagination info.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ResultInfo {
    page: u32,
    per_page: u32,
    total_pages: u32,
    count: u32,
    total_count: u32,
}

// =============================================================================
// Zone Types
// =============================================================================

/// A Cloudflare zone.
#[derive(Debug, Clone, Deserialize)]
pub struct Zone {
    /// Zone ID (32-character hex string).
    pub id: String,
    /// Domain name.
    pub name: String,
    /// Zone status.
    pub status: String,
    /// Whether the zone is paused.
    #[serde(default)]
    pub paused: bool,
    /// Zone type (full, partial, secondary).
    #[serde(rename = "type")]
    pub zone_type: Option<String>,
}

// =============================================================================
// DNS Record Types
// =============================================================================

/// A DNS record from Cloudflare.
#[derive(Debug, Clone, Deserialize)]
pub struct DnsRecord {
    /// Record ID (32-character hex string).
    pub id: String,
    /// Zone ID.
    #[serde(default)]
    pub zone_id: Option<String>,
    /// Zone name.
    #[serde(default)]
    pub zone_name: Option<String>,
    /// DNS record name (e.g., "example.com").
    pub name: String,
    /// Record type (A, AAAA, CNAME, etc.).
    #[serde(rename = "type")]
    pub record_type: String,
    /// Record content/value.
    pub content: String,
    /// Whether the record is proxied through Cloudflare.
    #[serde(default)]
    pub proxied: bool,
    /// TTL in seconds. 1 = automatic.
    pub ttl: u32,
    /// Priority for MX/SRV records.
    #[serde(default)]
    pub priority: Option<u16>,
    /// Record comment.
    #[serde(default)]
    pub comment: Option<String>,
    /// Record tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// SRV record data.
    #[serde(default)]
    pub data: Option<SrvData>,
}

/// SRV record data structure.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SrvData {
    /// Service name (without underscore prefix).
    #[serde(default)]
    pub service: Option<String>,
    /// Protocol (without underscore prefix).
    #[serde(default)]
    pub proto: Option<String>,
    /// Record name.
    #[serde(default)]
    pub name: Option<String>,
    /// Priority.
    #[serde(default)]
    pub priority: Option<u16>,
    /// Weight.
    #[serde(default)]
    pub weight: Option<u16>,
    /// Port.
    #[serde(default)]
    pub port: Option<u16>,
    /// Target hostname.
    #[serde(default)]
    pub target: Option<String>,
}

/// Request body for creating/updating DNS records.
#[derive(Debug, Serialize)]
pub struct CreateRecordRequest {
    /// Record type.
    #[serde(rename = "type")]
    pub record_type: String,
    /// DNS record name (e.g., "subdomain" or "subdomain.example.com").
    pub name: String,
    /// Record content/value.
    pub content: String,
    /// TTL in seconds. 1 = automatic.
    pub ttl: u32,
    /// Whether to proxy through Cloudflare.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxied: Option<bool>,
    /// Priority for MX records.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u16>,
    /// SRV record data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<SrvData>,
}

/// Supported DNS record types for Cloudflare.
const SUPPORTED_RECORD_TYPES: &[&str] = &["A", "AAAA", "CNAME", "MX", "NS", "TXT", "SRV"];

/// Error returned when a record cannot be converted to a [`crate::Record`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordConversionError {
    /// The record type that failed to convert.
    pub record_type: String,
    /// Description of what went wrong.
    pub reason: &'static str,
}

impl std::fmt::Display for RecordConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "failed to convert {} record: {}",
            self.record_type, self.reason
        )
    }
}

impl std::error::Error for RecordConversionError {}

/// A DNS record with its associated zone name for conversion.
///
/// This wrapper is used to implement `TryFrom` for converting Cloudflare
/// DNS records to generic [`crate::Record`] types, since the zone name
/// is needed to extract the subdomain.
pub struct DnsRecordWithZone<'a> {
    /// The DNS record.
    pub record: &'a DnsRecord,
    /// The zone domain name.
    pub zone_name: &'a str,
}

impl<'a> DnsRecordWithZone<'a> {
    /// Creates a new record-with-zone wrapper.
    pub fn new(record: &'a DnsRecord, zone_name: &'a str) -> Self {
        Self { record, zone_name }
    }
}

impl TryFrom<DnsRecordWithZone<'_>> for crate::Record {
    type Error = RecordConversionError;

    fn try_from(value: DnsRecordWithZone<'_>) -> Result<Self, Self::Error> {
        use crate::RecordData;

        let record = value.record;
        let zone_name = value.zone_name;

        // Extract the subdomain from the full record name
        let host = if record.name == zone_name {
            "@".to_string()
        } else if record.name.ends_with(&format!(".{}", zone_name)) {
            record.name[..record.name.len() - zone_name.len() - 1].to_string()
        } else {
            record.name.clone()
        };

        let data =
            match record.record_type.as_str() {
                "A" => record.content.parse().map(RecordData::A).map_err(|_| {
                    RecordConversionError {
                        record_type: record.record_type.clone(),
                        reason: "invalid IPv4 address",
                    }
                })?,
                "AAAA" => record.content.parse().map(RecordData::AAAA).map_err(|_| {
                    RecordConversionError {
                        record_type: record.record_type.clone(),
                        reason: "invalid IPv6 address",
                    }
                })?,
                "CNAME" => RecordData::CNAME(record.content.clone()),
                "MX" => RecordData::MX {
                    priority: record.priority.unwrap_or(10),
                    mail_server: record.content.clone(),
                },
                "NS" => RecordData::NS(record.content.clone()),
                "TXT" => RecordData::TXT(record.content.clone()),
                "SRV" => {
                    // SRV records have structured data
                    if let Some(data) = &record.data {
                        RecordData::SRV {
                            priority: data.priority.unwrap_or(0),
                            weight: data.weight.unwrap_or(0),
                            port: data.port.unwrap_or(0),
                            target: data.target.clone().unwrap_or_default(),
                        }
                    } else {
                        // Fallback: parse from content "priority weight port target"
                        let parts: Vec<&str> = record.content.split_whitespace().collect();
                        if parts.len() >= 4 {
                            RecordData::SRV {
                                priority: parts[0].parse().unwrap_or(0),
                                weight: parts[1].parse().unwrap_or(0),
                                port: parts[2].parse().unwrap_or(0),
                                target: parts[3].to_string(),
                            }
                        } else {
                            return Err(RecordConversionError {
                                record_type: record.record_type.clone(),
                                reason: "SRV record requires 4 parts: priority weight port target",
                            });
                        }
                    }
                }
                _ => {
                    return Err(RecordConversionError {
                        record_type: record.record_type.clone(),
                        reason: "unsupported record type",
                    });
                }
            };

        Ok(crate::Record {
            id: record.id.clone(),
            host,
            data,
            ttl: record.ttl as u64,
        })
    }
}

/// Error returned when building a [`CreateRecordRequest`] fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateRequestError {
    /// The record type is not supported by Cloudflare.
    UnsupportedType,
}

impl CreateRecordRequest {
    /// Creates a new record request from generic record data.
    ///
    /// # Arguments
    ///
    /// * `host` - The subdomain or "@" for the zone apex.
    /// * `data` - The record data.
    /// * `ttl` - Time to live in seconds. Use 0 for automatic.
    /// * `zone_name` - The zone domain name.
    ///
    /// # Returns
    ///
    /// Returns an error if the record type is not supported.
    pub fn from_record_data(
        host: &str,
        data: &crate::RecordData,
        ttl: u64,
        zone_name: &str,
    ) -> Result<Self, CreateRequestError> {
        use crate::RecordData;

        // Build the full record name
        let name = if host == "@" || host.is_empty() {
            zone_name.to_string()
        } else if host.ends_with(zone_name) {
            host.to_string()
        } else {
            format!("{}.{}", host, zone_name)
        };

        let (record_type, content, priority, srv_data) = match data {
            RecordData::A(ip) => ("A".to_string(), ip.to_string(), None, None),
            RecordData::AAAA(ip) => ("AAAA".to_string(), ip.to_string(), None, None),
            RecordData::CNAME(target) => ("CNAME".to_string(), target.clone(), None, None),
            RecordData::MX {
                priority,
                mail_server,
            } => ("MX".to_string(), mail_server.clone(), Some(*priority), None),
            RecordData::NS(ns) => ("NS".to_string(), ns.clone(), None, None),
            RecordData::TXT(txt) => ("TXT".to_string(), txt.clone(), None, None),
            RecordData::SRV {
                priority,
                weight,
                port,
                target,
            } => {
                // For SRV records, we need to use the data field
                let srv_data = SrvData {
                    service: None, // Parsed from name by Cloudflare
                    proto: None,   // Parsed from name by Cloudflare
                    name: None,
                    priority: Some(*priority),
                    weight: Some(*weight),
                    port: Some(*port),
                    target: Some(target.clone()),
                };
                // Content format for SRV: "weight port target"
                let content = format!("{} {} {}", weight, port, target);
                ("SRV".to_string(), content, Some(*priority), Some(srv_data))
            }
            RecordData::Other { typ, value } => {
                // Pass through other record types as-is
                (typ.clone(), value.clone(), None, None)
            }
        };

        // Check if record type is supported
        if !SUPPORTED_RECORD_TYPES.contains(&record_type.as_str()) {
            return Err(CreateRequestError::UnsupportedType);
        }

        Ok(Self {
            record_type,
            name,
            content,
            ttl: if ttl == 0 { 1 } else { ttl as u32 }, // 1 = automatic TTL
            proxied: Some(false),                       // Don't proxy DNS records by default
            priority,
            data: srv_data,
        })
    }
}

/// Delete response.
#[derive(Debug, Deserialize)]
pub struct DeleteResponse {
    pub id: String,
}

// =============================================================================
// API Client
// =============================================================================

/// Cloudflare API client.
pub struct Client {
    http_client: reqwest::Client,
    api_token: String,
    base_url: String,
}

impl Client {
    /// Creates a new Cloudflare API client.
    ///
    /// # Arguments
    ///
    /// * `api_token` - Cloudflare API token (Bearer token)
    pub fn new(api_token: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::with_base_url(api_token, CLOUDFLARE_API_URL, HttpClientConfig::default())
    }

    /// Creates a new Cloudflare API client with custom HTTP configuration.
    ///
    /// # Arguments
    ///
    /// * `api_token` - Cloudflare API token (Bearer token)
    /// * `config` - HTTP client configuration for network binding
    pub fn with_config(
        api_token: &str,
        config: HttpClientConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::with_base_url(api_token, CLOUDFLARE_API_URL, config)
    }

    /// Creates a new Cloudflare API client with a custom base URL.
    ///
    /// This is primarily useful for testing with mock servers.
    ///
    /// # Arguments
    ///
    /// * `api_token` - Cloudflare API token (Bearer token)
    /// * `base_url` - Custom base URL for the API
    /// * `config` - HTTP client configuration for network binding
    pub fn with_base_url(
        api_token: &str,
        base_url: &str,
        config: HttpClientConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut builder = reqwest::Client::builder()
            .timeout(config.timeout.unwrap_or(std::time::Duration::from_secs(30)));

        if let Some(addr) = config.local_address {
            builder = builder.local_address(addr);
        }

        #[cfg(any(
            target_os = "android",
            target_os = "fuchsia",
            target_os = "linux",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "illumos",
            target_os = "solaris",
        ))]
        if let Some(ref iface) = config.interface {
            builder = builder.interface(iface);
        }

        let http_client = builder.build()?;

        Ok(Self {
            http_client,
            api_token: api_token.to_string(),
            base_url: base_url.to_string(),
        })
    }

    /// Build headers for API requests.
    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.api_token)).unwrap(),
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers
    }

    /// Make a GET request.
    async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T, CloudflareError> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .http_client
            .get(&url)
            .headers(self.headers())
            .send()
            .await?;

        let api_response: ApiResponse<T> = response.json().await?;

        if !api_response.success {
            let error = api_response.errors.into_iter().next().unwrap_or(ApiError {
                code: 0,
                message: "Unknown error".to_string(),
            });
            return Err(CloudflareError::Api(error));
        }

        api_response.result.ok_or_else(|| {
            CloudflareError::Api(ApiError {
                code: 0,
                message: "No result in response".to_string(),
            })
        })
    }

    /// Make a paginated GET request returning a list.
    async fn get_list<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
    ) -> Result<Vec<T>, CloudflareError> {
        let mut all_results = Vec::new();
        let mut page = 1u32;

        loop {
            let url = if path.contains('?') {
                format!("{}{}&page={}&per_page=100", self.base_url, path, page)
            } else {
                format!("{}{}?page={}&per_page=100", self.base_url, path, page)
            };

            let response = self
                .http_client
                .get(&url)
                .headers(self.headers())
                .send()
                .await?;

            let api_response: ApiResponse<Vec<T>> = response.json().await?;

            if !api_response.success {
                let error = api_response.errors.into_iter().next().unwrap_or(ApiError {
                    code: 0,
                    message: "Unknown error".to_string(),
                });
                return Err(CloudflareError::Api(error));
            }

            if let Some(results) = api_response.result {
                let count = results.len();
                all_results.extend(results);

                // Check if we have more pages
                if let Some(info) = api_response.result_info {
                    if page >= info.total_pages || count == 0 {
                        break;
                    }
                    page += 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(all_results)
    }

    /// Make a POST request.
    async fn post<Req: Serialize, Resp: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &Req,
    ) -> Result<Resp, CloudflareError> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .http_client
            .post(&url)
            .headers(self.headers())
            .json(body)
            .send()
            .await?;

        let api_response: ApiResponse<Resp> = response.json().await?;

        if !api_response.success {
            let error = api_response.errors.into_iter().next().unwrap_or(ApiError {
                code: 0,
                message: "Unknown error".to_string(),
            });
            return Err(CloudflareError::Api(error));
        }

        api_response.result.ok_or_else(|| {
            CloudflareError::Api(ApiError {
                code: 0,
                message: "No result in response".to_string(),
            })
        })
    }

    /// Make a DELETE request.
    async fn delete<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T, CloudflareError> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .http_client
            .delete(&url)
            .headers(self.headers())
            .send()
            .await?;

        let api_response: ApiResponse<T> = response.json().await?;

        if !api_response.success {
            let error = api_response.errors.into_iter().next().unwrap_or(ApiError {
                code: 0,
                message: "Unknown error".to_string(),
            });
            return Err(CloudflareError::Api(error));
        }

        api_response.result.ok_or_else(|| {
            CloudflareError::Api(ApiError {
                code: 0,
                message: "No result in response".to_string(),
            })
        })
    }

    // =========================================================================
    // Zone APIs
    // =========================================================================

    /// Lists all zones accessible by the API token.
    pub async fn list_zones(&self) -> Result<Vec<Zone>, CloudflareError> {
        self.get_list("/zones").await
    }

    /// Gets a zone by ID.
    pub async fn get_zone(&self, zone_id: &str) -> Result<Zone, CloudflareError> {
        self.get(&format!("/zones/{}", zone_id)).await
    }

    /// Gets a zone by name (domain).
    pub async fn get_zone_by_name(&self, name: &str) -> Result<Zone, CloudflareError> {
        let zones: Vec<Zone> = self.get_list(&format!("/zones?name={}", name)).await?;
        zones.into_iter().next().ok_or_else(|| {
            CloudflareError::Api(ApiError {
                code: 1003,
                message: format!("Zone '{}' not found", name),
            })
        })
    }

    // =========================================================================
    // DNS Record APIs
    // =========================================================================

    /// Lists all DNS records in a zone.
    pub async fn list_records(&self, zone_id: &str) -> Result<Vec<DnsRecord>, CloudflareError> {
        self.get_list(&format!("/zones/{}/dns_records", zone_id))
            .await
    }

    /// Gets a DNS record by ID.
    pub async fn get_record(
        &self,
        zone_id: &str,
        record_id: &str,
    ) -> Result<DnsRecord, CloudflareError> {
        self.get(&format!("/zones/{}/dns_records/{}", zone_id, record_id))
            .await
    }

    /// Creates a new DNS record.
    pub async fn create_record(
        &self,
        zone_id: &str,
        request: &CreateRecordRequest,
    ) -> Result<DnsRecord, CloudflareError> {
        self.post(&format!("/zones/{}/dns_records", zone_id), request)
            .await
    }

    /// Deletes a DNS record.
    pub async fn delete_record(
        &self,
        zone_id: &str,
        record_id: &str,
    ) -> Result<DeleteResponse, CloudflareError> {
        self.delete(&format!("/zones/{}/dns_records/{}", zone_id, record_id))
            .await
    }
}
