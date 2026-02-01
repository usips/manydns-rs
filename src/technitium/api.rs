//! Technitium DNS Server API client implementation.
//!
//! This module provides the low-level HTTP API client for interacting with
//! Technitium DNS Server. The API uses token-based authentication which can
//! be obtained via login or by creating a non-expiring API token.

use reqwest::Client as HttpClient;
use serde::Deserialize;

use crate::HttpClientConfig;

/// The default port for Technitium DNS Server web interface.
pub const DEFAULT_PORT: u16 = 5380;

/// URL-encode a string for use in query parameters.
fn url_encode(s: &str) -> String {
    let mut encoded = String::new();
    for c in s.chars() {
        match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => encoded.push(c),
            ' ' => encoded.push_str("%20"),
            _ => {
                for byte in c.to_string().as_bytes() {
                    encoded.push_str(&format!("%{:02X}", byte));
                }
            }
        }
    }
    encoded
}

/// API client for Technitium DNS Server.
#[derive(Debug, Clone)]
pub struct Client {
    http_client: HttpClient,
    base_url: String,
    token: String,
}

impl Client {
    /// Creates a new API client with the given base URL and API token.
    ///
    /// The base URL should be in the format `http://hostname:port` or `https://hostname:port`.
    /// Do not include a trailing slash.
    ///
    /// The token can be obtained by:
    /// - Logging in via `/api/user/login`
    /// - Creating a non-expiring API token via `/api/user/createToken`
    pub fn new(base_url: &str, token: &str) -> Result<Self, reqwest::Error> {
        Self::with_config(base_url, token, HttpClientConfig::default())
    }

    /// Creates a new API client with custom HTTP configuration.
    ///
    /// # Arguments
    ///
    /// * `base_url` - The base URL of the Technitium DNS Server
    /// * `token` - The API token for authentication
    /// * `config` - HTTP client configuration for network binding
    pub fn with_config(
        base_url: &str,
        token: &str,
        config: HttpClientConfig,
    ) -> Result<Self, reqwest::Error> {
        let mut builder = HttpClient::builder();

        if let Some(timeout) = config.timeout {
            builder = builder.timeout(timeout);
        }

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
            base_url: base_url.trim_end_matches('/').to_string(),
            token: token.to_string(),
        })
    }

    /// Creates a new API client by logging in with username and password.
    ///
    /// This will create a session token that expires after 30 minutes of inactivity.
    /// For long-running applications, consider using a non-expiring API token instead.
    pub async fn login(base_url: &str, username: &str, password: &str) -> Result<Self, ApiError> {
        let http_client = HttpClient::builder().build().map_err(ApiError::Request)?;
        let base_url = base_url.trim_end_matches('/').to_string();

        let response: LoginResponse = http_client
            .get(format!(
                "{}/api/user/login?user={}&pass={}",
                base_url, username, password
            ))
            .send()
            .await
            .map_err(ApiError::Request)?
            .json()
            .await
            .map_err(ApiError::Request)?;

        match response.status {
            ApiStatus::Ok => Ok(Self {
                http_client,
                base_url,
                token: response.token.ok_or(ApiError::MissingToken)?,
            }),
            ApiStatus::InvalidToken => Err(ApiError::InvalidToken),
            ApiStatus::TwoFactorRequired => Err(ApiError::TwoFactorRequired),
            ApiStatus::Error => Err(ApiError::ApiStatus(
                response.error_message.unwrap_or_default(),
            )),
        }
    }

    fn url_with_token(&self, path: &str) -> String {
        if path.contains('?') {
            format!("{}{}&token={}", self.base_url, path, self.token)
        } else {
            format!("{}{}?token={}", self.base_url, path, self.token)
        }
    }

    /// Lists all authoritative zones.
    pub async fn list_zones(&self) -> Result<ZonesResponse, ApiError> {
        let response: ApiResponse<ZonesResponse> = self
            .http_client
            .get(self.url_with_token("/api/zones/list"))
            .send()
            .await
            .map_err(ApiError::Request)?
            .json()
            .await
            .map_err(ApiError::Request)?;

        response.into_result()
    }

    /// Gets a zone by its domain name.
    ///
    /// Note: Technitium uses the zone domain name as the identifier.
    pub async fn get_zone(&self, zone: &str) -> Result<ZoneOptionsResponse, ApiError> {
        let response: ApiResponse<ZoneOptionsResponse> = self
            .http_client
            .get(self.url_with_token(&format!("/api/zones/options/get?zone={}", zone)))
            .send()
            .await
            .map_err(ApiError::Request)?
            .json()
            .await
            .map_err(ApiError::Request)?;

        response.into_result()
    }

    /// Creates a new primary zone.
    pub async fn create_zone(&self, zone: &str) -> Result<CreateZoneResponse, ApiError> {
        let response: ApiResponse<CreateZoneResponse> = self
            .http_client
            .get(self.url_with_token(&format!("/api/zones/create?zone={}&type=Primary", zone)))
            .send()
            .await
            .map_err(ApiError::Request)?
            .json()
            .await
            .map_err(ApiError::Request)?;

        response.into_result()
    }

    /// Deletes a zone.
    pub async fn delete_zone(&self, zone: &str) -> Result<(), ApiError> {
        let response: EmptyApiResponse = self
            .http_client
            .get(self.url_with_token(&format!("/api/zones/delete?zone={}", zone)))
            .send()
            .await
            .map_err(ApiError::Request)?
            .json()
            .await
            .map_err(ApiError::Request)?;

        response.into_result()
    }

    /// Enables a zone.
    pub async fn enable_zone(&self, zone: &str) -> Result<(), ApiError> {
        let response: EmptyApiResponse = self
            .http_client
            .get(self.url_with_token(&format!("/api/zones/enable?zone={}", zone)))
            .send()
            .await
            .map_err(ApiError::Request)?
            .json()
            .await
            .map_err(ApiError::Request)?;

        response.into_result()
    }

    /// Disables a zone.
    pub async fn disable_zone(&self, zone: &str) -> Result<(), ApiError> {
        let response: EmptyApiResponse = self
            .http_client
            .get(self.url_with_token(&format!("/api/zones/disable?zone={}", zone)))
            .send()
            .await
            .map_err(ApiError::Request)?
            .json()
            .await
            .map_err(ApiError::Request)?;

        response.into_result()
    }

    /// Lists all records in a zone.
    pub async fn list_records(&self, zone: &str) -> Result<RecordsResponse, ApiError> {
        let response: ApiResponse<RecordsResponse> = self
            .http_client
            .get(self.url_with_token(&format!(
                "/api/zones/records/get?domain={}&zone={}&listZone=true",
                zone, zone
            )))
            .send()
            .await
            .map_err(ApiError::Request)?
            .json()
            .await
            .map_err(ApiError::Request)?;

        response.into_result()
    }

    /// Gets records for a specific domain within a zone.
    pub async fn get_records(&self, zone: &str, domain: &str) -> Result<RecordsResponse, ApiError> {
        let response: ApiResponse<RecordsResponse> = self
            .http_client
            .get(self.url_with_token(&format!(
                "/api/zones/records/get?domain={}&zone={}",
                domain, zone
            )))
            .send()
            .await
            .map_err(ApiError::Request)?
            .json()
            .await
            .map_err(ApiError::Request)?;

        response.into_result()
    }

    /// Adds a new DNS record.
    pub async fn add_record(
        &self,
        zone: &str,
        domain: &str,
        record_type: &str,
        ttl: u64,
        record_params: &RecordParams,
    ) -> Result<AddRecordResponse, ApiError> {
        let mut url = format!(
            "/api/zones/records/add?domain={}&zone={}&type={}&ttl={}",
            domain, zone, record_type, ttl
        );

        // Add record-type specific parameters
        match record_params {
            RecordParams::A { ip_address } => {
                url.push_str(&format!("&ipAddress={}", ip_address));
            }
            RecordParams::AAAA { ip_address } => {
                url.push_str(&format!("&ipAddress={}", ip_address));
            }
            RecordParams::CNAME { cname } => {
                url.push_str(&format!("&cname={}", cname));
            }
            RecordParams::MX {
                preference,
                exchange,
            } => {
                url.push_str(&format!("&preference={}&exchange={}", preference, exchange));
            }
            RecordParams::NS { name_server } => {
                url.push_str(&format!("&nameServer={}", name_server));
            }
            RecordParams::TXT { text } => {
                url.push_str(&format!("&text={}", url_encode(text)));
            }
            RecordParams::SRV {
                priority,
                weight,
                port,
                target,
            } => {
                url.push_str(&format!(
                    "&priority={}&weight={}&port={}&target={}",
                    priority, weight, port, target
                ));
            }
            RecordParams::PTR { ptr_name } => {
                url.push_str(&format!("&ptrName={}", ptr_name));
            }
            RecordParams::CAA { flags, tag, value } => {
                url.push_str(&format!(
                    "&flags={}&tag={}&value={}",
                    flags,
                    tag,
                    url_encode(value)
                ));
            }
            RecordParams::DS {
                key_tag,
                algorithm,
                digest_type,
                digest,
            } => {
                url.push_str(&format!(
                    "&keyTag={}&algorithm={}&digestType={}&digest={}",
                    key_tag, algorithm, digest_type, digest
                ));
            }
            RecordParams::DNAME { dname } => {
                url.push_str(&format!("&dname={}", dname));
            }
            RecordParams::Other { value } => {
                url.push_str(&format!("&rdata={}", url_encode(value)));
            }
        }

        let response: ApiResponse<AddRecordResponse> = self
            .http_client
            .get(self.url_with_token(&url))
            .send()
            .await
            .map_err(ApiError::Request)?
            .json()
            .await
            .map_err(ApiError::Request)?;

        response.into_result()
    }

    /// Deletes a DNS record.
    pub async fn delete_record(
        &self,
        zone: &str,
        domain: &str,
        record_type: &str,
        record_params: &RecordParams,
    ) -> Result<(), ApiError> {
        let mut url = format!(
            "/api/zones/records/delete?domain={}&zone={}&type={}",
            domain, zone, record_type
        );

        // Add record-type specific parameters for identifying the record
        match record_params {
            RecordParams::A { ip_address } => {
                url.push_str(&format!("&ipAddress={}", ip_address));
            }
            RecordParams::AAAA { ip_address } => {
                url.push_str(&format!("&ipAddress={}", ip_address));
            }
            RecordParams::CNAME { cname: _ } => {
                // CNAME doesn't need additional params for delete
            }
            RecordParams::MX {
                preference,
                exchange,
            } => {
                url.push_str(&format!("&preference={}&exchange={}", preference, exchange));
            }
            RecordParams::NS { name_server } => {
                url.push_str(&format!("&nameServer={}", name_server));
            }
            RecordParams::TXT { text } => {
                url.push_str(&format!("&text={}", url_encode(text)));
            }
            RecordParams::SRV {
                priority,
                weight,
                port,
                target,
            } => {
                url.push_str(&format!(
                    "&priority={}&weight={}&port={}&target={}",
                    priority, weight, port, target
                ));
            }
            RecordParams::PTR { ptr_name } => {
                url.push_str(&format!("&ptrName={}", ptr_name));
            }
            RecordParams::CAA { flags, tag, value } => {
                url.push_str(&format!(
                    "&flags={}&tag={}&value={}",
                    flags,
                    tag,
                    url_encode(value)
                ));
            }
            RecordParams::DS {
                key_tag,
                algorithm,
                digest_type,
                digest,
            } => {
                url.push_str(&format!(
                    "&keyTag={}&algorithm={}&digestType={}&digest={}",
                    key_tag, algorithm, digest_type, digest
                ));
            }
            RecordParams::DNAME { dname } => {
                url.push_str(&format!("&dname={}", dname));
            }
            RecordParams::Other { value } => {
                url.push_str(&format!("&rdata={}", url_encode(value)));
            }
        }

        let response: EmptyApiResponse = self
            .http_client
            .get(self.url_with_token(&url))
            .send()
            .await
            .map_err(ApiError::Request)?
            .json()
            .await
            .map_err(ApiError::Request)?;

        response.into_result()
    }
}

/// Parameters for different record types.
#[derive(Debug, Clone)]
pub enum RecordParams {
    A {
        ip_address: String,
    },
    AAAA {
        ip_address: String,
    },
    CNAME {
        cname: String,
    },
    MX {
        preference: u16,
        exchange: String,
    },
    NS {
        name_server: String,
    },
    TXT {
        text: String,
    },
    SRV {
        priority: u16,
        weight: u16,
        port: u16,
        target: String,
    },
    PTR {
        ptr_name: String,
    },
    CAA {
        flags: u8,
        tag: String,
        value: String,
    },
    DS {
        key_tag: u16,
        algorithm: String,
        digest_type: String,
        digest: String,
    },
    DNAME {
        dname: String,
    },
    Other {
        value: String,
    },
}

/// API response status as documented by Technitium.
///
/// The `status` property can have the following values:
/// - `ok`: The call was successful.
/// - `error`: The call failed (additional error properties provided).
/// - `invalid-token`: Session expired or invalid token provided.
/// - `2fa-required`: Two-factor authentication OTP was not provided.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApiStatus {
    Ok,
    Error,
    InvalidToken,
    #[serde(rename = "2fa-required")]
    TwoFactorRequired,
}

/// API error types.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("API returned error status: {0}")]
    ApiStatus(String),

    #[error("Missing token in login response")]
    MissingToken,

    #[error("Invalid or expired token")]
    InvalidToken,

    #[error("Two-factor authentication required")]
    TwoFactorRequired,

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Not found")]
    NotFound,

    #[error("Invalid domain name")]
    InvalidDomainName,

    #[error("Invalid record")]
    InvalidRecord,
}

/// Generic API response wrapper.
#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    pub status: ApiStatus,
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
    pub response: Option<T>,
}

impl<T> ApiResponse<T> {
    pub fn into_result(self) -> Result<T, ApiError> {
        match self.status {
            ApiStatus::Ok => self
                .response
                .ok_or(ApiError::ApiStatus("Missing response data".to_string())),
            ApiStatus::InvalidToken => Err(ApiError::InvalidToken),
            ApiStatus::TwoFactorRequired => Err(ApiError::TwoFactorRequired),
            ApiStatus::Error => {
                let msg = self.error_message.unwrap_or_default();
                Err(classify_error_message(&msg))
            }
        }
    }
}

/// API response without a response body (for delete/enable/disable operations).
#[derive(Debug, Deserialize)]
pub struct EmptyApiResponse {
    pub status: ApiStatus,
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
}

impl EmptyApiResponse {
    pub fn into_result(self) -> Result<(), ApiError> {
        match self.status {
            ApiStatus::Ok => Ok(()),
            ApiStatus::InvalidToken => Err(ApiError::InvalidToken),
            ApiStatus::TwoFactorRequired => Err(ApiError::TwoFactorRequired),
            ApiStatus::Error => {
                let msg = self.error_message.unwrap_or_default();
                Err(classify_error_message(&msg))
            }
        }
    }
}

/// Classify an error message into a specific error type.
///
/// Note: This is still string-based because Technitium's API doesn't provide
/// structured error codes—only human-readable error messages. This is the best
/// we can do without upstream API changes.
fn classify_error_message(msg: &str) -> ApiError {
    let msg_lower = msg.to_lowercase();
    if msg_lower.contains("not authorized") || msg_lower.contains("unauthorized") {
        ApiError::Unauthorized
    } else if msg_lower.contains("not found")
        || msg_lower.contains("does not exist")
        || msg_lower.contains("no such zone")
    {
        ApiError::NotFound
    } else if msg_lower.contains("invalid") && msg_lower.contains("domain") {
        ApiError::InvalidDomainName
    } else {
        ApiError::ApiStatus(msg.to_string())
    }
}

/// Login response.
#[derive(Debug, Deserialize)]
pub struct LoginResponse {
    pub status: ApiStatus,
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
    pub token: Option<String>,
    pub username: Option<String>,
}

/// Zones list response.
#[derive(Debug, Deserialize)]
pub struct ZonesResponse {
    pub zones: Vec<Zone>,
    #[serde(rename = "pageNumber")]
    pub page_number: Option<u32>,
    #[serde(rename = "totalPages")]
    pub total_pages: Option<u32>,
    #[serde(rename = "totalZones")]
    pub total_zones: Option<u32>,
}

/// Zone information.
#[derive(Debug, Clone, Deserialize)]
pub struct Zone {
    pub name: String,
    #[serde(rename = "type")]
    pub zone_type: String,
    #[serde(default)]
    pub internal: bool,
    #[serde(rename = "dnssecStatus")]
    pub dnssec_status: Option<String>,
    #[serde(rename = "soaSerial")]
    pub soa_serial: Option<u64>,
    #[serde(default)]
    pub disabled: bool,
    #[serde(rename = "lastModified")]
    pub last_modified: Option<String>,
}

/// Zone options response (used for get_zone).
#[derive(Debug, Deserialize)]
pub struct ZoneOptionsResponse {
    pub name: String,
    #[serde(rename = "type")]
    pub zone_type: String,
    #[serde(default)]
    pub internal: bool,
    #[serde(rename = "dnssecStatus")]
    pub dnssec_status: Option<String>,
    #[serde(default)]
    pub disabled: bool,
}

/// Create zone response.
#[derive(Debug, Deserialize)]
pub struct CreateZoneResponse {
    pub domain: String,
}

/// Records response.
#[derive(Debug, Deserialize)]
pub struct RecordsResponse {
    pub zone: ZoneInfo,
    pub records: Vec<Record>,
}

/// Zone info in records response.
#[derive(Debug, Deserialize)]
pub struct ZoneInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub zone_type: String,
    #[serde(default)]
    pub disabled: bool,
}

/// DNS record.
#[derive(Debug, Clone, Deserialize)]
pub struct Record {
    #[serde(default)]
    pub disabled: bool,
    pub name: String,
    #[serde(rename = "type")]
    pub record_type: String,
    pub ttl: u64,
    #[serde(rename = "rData")]
    pub rdata: RecordData,
}

/// Record data (varies by record type).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum RecordData {
    A {
        #[serde(rename = "ipAddress")]
        ip_address: String,
    },
    AAAA {
        #[serde(rename = "ipAddress")]
        ip_address: String,
    },
    CNAME {
        cname: String,
    },
    MX {
        preference: u16,
        exchange: String,
    },
    NS {
        #[serde(rename = "nameServer")]
        name_server: String,
    },
    TXT {
        text: String,
    },
    SRV {
        priority: u16,
        weight: u16,
        port: u16,
        target: String,
    },
    PTR {
        #[serde(rename = "ptrName")]
        ptr_name: String,
    },
    SOA {
        #[serde(rename = "primaryNameServer")]
        primary_name_server: String,
        #[serde(rename = "responsiblePerson")]
        responsible_person: String,
        serial: u64,
        refresh: u64,
        retry: u64,
        expire: u64,
        minimum: u64,
    },
    CAA {
        flags: u8,
        tag: String,
        value: String,
    },
    DS {
        #[serde(rename = "keyTag")]
        key_tag: u16,
        algorithm: String,
        #[serde(rename = "digestType")]
        digest_type: String,
        digest: String,
    },
    DNAME {
        dname: String,
    },
    /// Fallback for unknown record types
    Other(serde_json::Value),
}

impl RecordData {
    /// Converts API record data to the generic library format.
    pub fn to_value_string(&self) -> String {
        match self {
            RecordData::A { ip_address } => ip_address.clone(),
            RecordData::AAAA { ip_address } => ip_address.clone(),
            RecordData::CNAME { cname } => cname.clone(),
            RecordData::MX {
                preference,
                exchange,
            } => format!("{} {}", preference, exchange),
            RecordData::NS { name_server } => name_server.clone(),
            RecordData::TXT { text } => text.clone(),
            RecordData::SRV {
                priority,
                weight,
                port,
                target,
            } => format!("{} {} {} {}", priority, weight, port, target),
            RecordData::PTR { ptr_name } => ptr_name.clone(),
            RecordData::SOA {
                primary_name_server,
                responsible_person,
                serial,
                refresh,
                retry,
                expire,
                minimum,
            } => format!(
                "{} {} {} {} {} {} {}",
                primary_name_server, responsible_person, serial, refresh, retry, expire, minimum
            ),
            RecordData::CAA { flags, tag, value } => format!("{} {} \"{}\"", flags, tag, value),
            RecordData::DS {
                key_tag,
                algorithm,
                digest_type,
                digest,
            } => format!("{} {} {} {}", key_tag, algorithm, digest_type, digest),
            RecordData::DNAME { dname } => dname.clone(),
            RecordData::Other(v) => v.to_string(),
        }
    }

    /// Converts to RecordParams for API calls.
    pub fn to_params(&self) -> RecordParams {
        match self {
            RecordData::A { ip_address } => RecordParams::A {
                ip_address: ip_address.clone(),
            },
            RecordData::AAAA { ip_address } => RecordParams::AAAA {
                ip_address: ip_address.clone(),
            },
            RecordData::CNAME { cname } => RecordParams::CNAME {
                cname: cname.clone(),
            },
            RecordData::MX {
                preference,
                exchange,
            } => RecordParams::MX {
                preference: *preference,
                exchange: exchange.clone(),
            },
            RecordData::NS { name_server } => RecordParams::NS {
                name_server: name_server.clone(),
            },
            RecordData::TXT { text } => RecordParams::TXT { text: text.clone() },
            RecordData::SRV {
                priority,
                weight,
                port,
                target,
            } => RecordParams::SRV {
                priority: *priority,
                weight: *weight,
                port: *port,
                target: target.clone(),
            },
            RecordData::PTR { ptr_name } => RecordParams::PTR {
                ptr_name: ptr_name.clone(),
            },
            RecordData::CAA { flags, tag, value } => RecordParams::CAA {
                flags: *flags,
                tag: tag.clone(),
                value: value.clone(),
            },
            RecordData::DS {
                key_tag,
                algorithm,
                digest_type,
                digest,
            } => RecordParams::DS {
                key_tag: *key_tag,
                algorithm: algorithm.clone(),
                digest_type: digest_type.clone(),
                digest: digest.clone(),
            },
            RecordData::DNAME { dname } => RecordParams::DNAME {
                dname: dname.clone(),
            },
            RecordData::SOA { .. } | RecordData::Other(_) => RecordParams::Other {
                value: self.to_value_string(),
            },
        }
    }
}

/// Add record response.
#[derive(Debug, Deserialize)]
pub struct AddRecordResponse {
    pub zone: ZoneInfo,
    #[serde(rename = "addedRecord")]
    pub added_record: Record,
}
