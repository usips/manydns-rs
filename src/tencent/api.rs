//! Low-level Tencent Cloud DNSPod API client.
//!
//! This module provides direct access to the Tencent Cloud DNSPod API using
//! the TC3-HMAC-SHA256 signature algorithm.
//!
//! # API Reference
//!
//! - [API Category](https://www.tencentcloud.com/document/api/1157/49025)
//! - [Signature v3](https://www.tencentcloud.com/document/api/1157/49029)
//! - [Data Types](https://www.tencentcloud.com/document/api/1157/49043)

use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

use crate::HttpClientConfig;

/// The Tencent Cloud DNSPod API endpoint.
const TENCENT_API_HOST: &str = "dnspod.intl.tencentcloudapi.com";
const TENCENT_API_URL: &str = "https://dnspod.intl.tencentcloudapi.com";

/// Service name for signature calculation.
const SERVICE: &str = "dnspod";

/// API version.
const API_VERSION: &str = "2021-03-23";

/// Errors that may occur when interacting with the Tencent Cloud API.
#[derive(Debug, Error)]
pub enum TencentError {
    /// The API returned an error response.
    #[error("API error: {0}")]
    Api(ApiError),

    /// An HTTP request error occurred.
    #[error("HTTP request error: {0}")]
    Request(#[from] reqwest::Error),

    /// Failed to serialize request.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Tencent Cloud API error response.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiError {
    /// Error code.
    #[serde(rename = "Code")]
    pub code: String,
    /// Error message.
    #[serde(rename = "Message")]
    pub message: String,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

/// Raw API response that can contain either an error or success data.
/// We first deserialize to this to check for errors before deserializing the actual data.
#[derive(Debug, Deserialize)]
struct RawApiResponse {
    #[serde(rename = "Response")]
    response: serde_json::Value,
}

/// Error response structure.
#[derive(Debug, Deserialize)]
struct ErrorResponse {
    #[serde(rename = "Error")]
    error: ApiError,
}

// =============================================================================
// API Request/Response Types
// =============================================================================

/// Domain list item from DescribeDomainList.
#[derive(Debug, Clone, Deserialize)]
pub struct DomainListItem {
    /// Domain ID.
    #[serde(rename = "DomainId")]
    pub domain_id: u64,
    /// Domain name.
    #[serde(rename = "Name")]
    pub name: String,
    /// Domain status.
    #[serde(rename = "Status")]
    pub status: String,
    /// TTL.
    #[serde(rename = "TTL")]
    pub ttl: u64,
    /// Record count.
    #[serde(rename = "RecordCount")]
    pub record_count: Option<u64>,
}

/// Response for DescribeDomainList.
#[derive(Debug, Deserialize)]
pub struct DescribeDomainListResponse {
    #[serde(rename = "DomainList")]
    pub domain_list: Option<Vec<DomainListItem>>,
    #[serde(rename = "DomainCountInfo")]
    pub domain_count_info: Option<DomainCountInfo>,
    #[serde(rename = "RequestId")]
    pub request_id: String,
}

/// Domain count information.
#[derive(Debug, Deserialize)]
pub struct DomainCountInfo {
    #[serde(rename = "DomainTotal")]
    pub domain_total: u64,
    #[serde(rename = "AllTotal")]
    pub all_total: u64,
}

/// Domain info from DescribeDomain.
#[derive(Debug, Clone, Deserialize)]
pub struct DomainInfo {
    /// Domain ID.
    #[serde(rename = "DomainId")]
    pub domain_id: u64,
    /// Domain name.
    #[serde(rename = "Domain")]
    pub domain: String,
    /// Domain status.
    #[serde(rename = "Status")]
    pub status: String,
    /// TTL.
    #[serde(rename = "TTL")]
    pub ttl: u64,
    /// Record count.
    #[serde(rename = "RecordCount")]
    pub record_count: Option<u64>,
}

/// Response for DescribeDomain.
#[derive(Debug, Deserialize)]
pub struct DescribeDomainResponse {
    #[serde(rename = "DomainInfo")]
    pub domain_info: DomainInfo,
    #[serde(rename = "RequestId")]
    pub request_id: String,
}

/// Record list item from DescribeRecordList.
#[derive(Debug, Clone, Deserialize)]
pub struct RecordListItem {
    /// Record ID.
    #[serde(rename = "RecordId")]
    pub record_id: u64,
    /// Subdomain (host record).
    #[serde(rename = "Name")]
    pub name: String,
    /// Record type.
    #[serde(rename = "Type")]
    pub record_type: String,
    /// Record value.
    #[serde(rename = "Value")]
    pub value: String,
    /// TTL.
    #[serde(rename = "TTL")]
    pub ttl: u64,
    /// Record line.
    #[serde(rename = "Line")]
    pub line: String,
    /// Record status.
    #[serde(rename = "Status")]
    pub status: String,
    /// MX priority.
    #[serde(rename = "MX")]
    pub mx: Option<u16>,
}

impl TryFrom<&RecordListItem> for crate::Record {
    type Error = RecordConversionError;

    fn try_from(item: &RecordListItem) -> Result<Self, Self::Error> {
        let data = parse_record_data(&item.record_type, &item.value, item.mx)?;
        Ok(crate::Record {
            id: item.record_id.to_string(),
            host: item.name.clone(),
            data,
            ttl: item.ttl,
        })
    }
}

impl TryFrom<RecordListItem> for crate::Record {
    type Error = RecordConversionError;

    fn try_from(item: RecordListItem) -> Result<Self, Self::Error> {
        Self::try_from(&item)
    }
}

/// Response for DescribeRecordList.
#[derive(Debug, Deserialize)]
pub struct DescribeRecordListResponse {
    #[serde(rename = "RecordList")]
    pub record_list: Option<Vec<RecordListItem>>,
    #[serde(rename = "RecordCountInfo")]
    pub record_count_info: Option<RecordCountInfo>,
    #[serde(rename = "RequestId")]
    pub request_id: String,
}

/// Record count information.
#[derive(Debug, Deserialize)]
pub struct RecordCountInfo {
    #[serde(rename = "TotalCount")]
    pub total_count: u64,
}

/// Record info from DescribeRecord.
#[derive(Debug, Clone, Deserialize)]
pub struct RecordInfo {
    /// Record ID.
    #[serde(rename = "Id")]
    pub id: u64,
    /// Subdomain.
    #[serde(rename = "SubDomain")]
    pub sub_domain: String,
    /// Record type.
    #[serde(rename = "RecordType")]
    pub record_type: String,
    /// Record value.
    #[serde(rename = "Value")]
    pub value: String,
    /// TTL.
    #[serde(rename = "TTL")]
    pub ttl: u64,
    /// MX priority.
    #[serde(rename = "MX")]
    pub mx: Option<u16>,
    /// Record status.
    #[serde(rename = "Enabled")]
    pub enabled: u8,
}

impl TryFrom<&RecordInfo> for crate::Record {
    type Error = RecordConversionError;

    fn try_from(info: &RecordInfo) -> Result<Self, Self::Error> {
        let data = parse_record_data(&info.record_type, &info.value, info.mx)?;
        Ok(crate::Record {
            id: info.id.to_string(),
            host: info.sub_domain.clone(),
            data,
            ttl: info.ttl,
        })
    }
}

impl TryFrom<RecordInfo> for crate::Record {
    type Error = RecordConversionError;

    fn try_from(info: RecordInfo) -> Result<Self, Self::Error> {
        Self::try_from(&info)
    }
}

/// Response for DescribeRecord.
#[derive(Debug, Deserialize)]
pub struct DescribeRecordResponse {
    #[serde(rename = "RecordInfo")]
    pub record_info: RecordInfo,
    #[serde(rename = "RequestId")]
    pub request_id: String,
}

/// Response for CreateRecord.
#[derive(Debug, Deserialize)]
pub struct CreateRecordResponse {
    #[serde(rename = "RecordId")]
    pub record_id: u64,
    #[serde(rename = "RequestId")]
    pub request_id: String,
}

/// Response for ModifyRecord.
#[derive(Debug, Deserialize)]
pub struct ModifyRecordResponse {
    #[serde(rename = "RecordId")]
    pub record_id: u64,
    #[serde(rename = "RequestId")]
    pub request_id: String,
}

/// Response for DeleteRecord.
#[derive(Debug, Deserialize)]
pub struct DeleteRecordResponse {
    #[serde(rename = "RequestId")]
    pub request_id: String,
}

/// Response for CreateDomain.
#[derive(Debug, Deserialize)]
pub struct CreateDomainResponse {
    #[serde(rename = "DomainInfo")]
    pub domain_info: DomainCreateInfo,
    #[serde(rename = "RequestId")]
    pub request_id: String,
}

/// Domain creation info.
#[derive(Debug, Clone, Deserialize)]
pub struct DomainCreateInfo {
    /// Domain ID.
    #[serde(rename = "Id")]
    pub id: u64,
    /// Domain name.
    #[serde(rename = "Domain")]
    pub domain: String,
}

/// Response for DeleteDomain.
#[derive(Debug, Deserialize)]
pub struct DeleteDomainResponse {
    #[serde(rename = "RequestId")]
    pub request_id: String,
}

// =============================================================================
// TC3-HMAC-SHA256 Signature Implementation
// =============================================================================

/// Computes SHA256 hash and returns it as a lowercase hex string.
fn sha256_hex(data: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    hex::encode(hasher.finalize())
}

/// Computes HMAC-SHA256 and returns the raw bytes.
fn hmac_sha256(key: &[u8], data: &str) -> Vec<u8> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(data.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

/// Generates the TC3-HMAC-SHA256 signature for a request.
fn generate_signature(
    secret_id: &str,
    secret_key: &str,
    timestamp: u64,
    _action: &str,
    payload: &str,
) -> (String, String) {
    // Step 1: Build canonical request
    let http_request_method = "POST";
    let canonical_uri = "/";
    let canonical_query_string = "";
    let content_type = "application/json; charset=utf-8";
    let canonical_headers = format!("content-type:{}\nhost:{}\n", content_type, TENCENT_API_HOST);
    let signed_headers = "content-type;host";
    let hashed_request_payload = sha256_hex(payload);

    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        http_request_method,
        canonical_uri,
        canonical_query_string,
        canonical_headers,
        signed_headers,
        hashed_request_payload
    );

    // Step 2: Build string to sign
    let algorithm = "TC3-HMAC-SHA256";
    let date = chrono::DateTime::from_timestamp(timestamp as i64, 0)
        .unwrap()
        .format("%Y-%m-%d")
        .to_string();
    let credential_scope = format!("{}/{}/tc3_request", date, SERVICE);
    let hashed_canonical_request = sha256_hex(&canonical_request);

    let string_to_sign = format!(
        "{}\n{}\n{}\n{}",
        algorithm, timestamp, credential_scope, hashed_canonical_request
    );

    // Step 3: Calculate signature
    let secret_date = hmac_sha256(format!("TC3{}", secret_key).as_bytes(), &date);
    let secret_service = hmac_sha256(&secret_date, SERVICE);
    let secret_signing = hmac_sha256(&secret_service, "tc3_request");
    let signature = hex::encode(hmac_sha256(&secret_signing, &string_to_sign));

    // Step 4: Build authorization header
    let authorization = format!(
        "{} Credential={}/{}, SignedHeaders={}, Signature={}",
        algorithm, secret_id, credential_scope, signed_headers, signature
    );

    (authorization, date)
}

// =============================================================================
// API Client
// =============================================================================

/// Tencent Cloud DNSPod API client.
pub struct Client {
    http_client: reqwest::Client,
    secret_id: String,
    secret_key: String,
}

impl Client {
    /// Creates a new Tencent Cloud API client.
    ///
    /// # Arguments
    ///
    /// * `secret_id` - Tencent Cloud SecretId from API key management
    /// * `secret_key` - Tencent Cloud SecretKey from API key management
    pub fn new(
        secret_id: &str,
        secret_key: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::with_config(secret_id, secret_key, HttpClientConfig::default())
    }

    /// Creates a new Tencent Cloud API client with custom HTTP configuration.
    ///
    /// # Arguments
    ///
    /// * `secret_id` - Tencent Cloud SecretId from API key management
    /// * `secret_key` - Tencent Cloud SecretKey from API key management
    /// * `config` - HTTP client configuration for network binding
    pub fn with_config(
        secret_id: &str,
        secret_key: &str,
        config: HttpClientConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut builder = reqwest::Client::builder();

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
            secret_id: secret_id.to_string(),
            secret_key: secret_key.to_string(),
        })
    }

    /// Makes a signed API request.
    async fn request<Req, Resp>(&self, action: &str, request: &Req) -> Result<Resp, TencentError>
    where
        Req: Serialize,
        Resp: for<'de> Deserialize<'de>,
    {
        let payload = serde_json::to_string(request)?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let (authorization, _date) = generate_signature(
            &self.secret_id,
            &self.secret_key,
            timestamp,
            action,
            &payload,
        );

        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=utf-8"),
        );
        headers.insert("Host", HeaderValue::from_static(TENCENT_API_HOST));
        headers.insert(
            "Authorization",
            HeaderValue::from_str(&authorization).unwrap(),
        );
        headers.insert("X-TC-Action", HeaderValue::from_str(action).unwrap());
        headers.insert("X-TC-Version", HeaderValue::from_static(API_VERSION));
        headers.insert(
            "X-TC-Timestamp",
            HeaderValue::from_str(&timestamp.to_string()).unwrap(),
        );

        let response = self
            .http_client
            .post(TENCENT_API_URL)
            .headers(headers)
            .body(payload)
            .send()
            .await?;

        // First, get the raw JSON to check for errors
        let raw: RawApiResponse = response.json().await?;

        // Check if the response contains an Error field
        if let Ok(error_resp) = serde_json::from_value::<ErrorResponse>(raw.response.clone()) {
            return Err(TencentError::Api(error_resp.error));
        }

        // No error, deserialize the success response
        let data: Resp = serde_json::from_value(raw.response)?;
        Ok(data)
    }

    // =========================================================================
    // Domain APIs
    // =========================================================================

    /// Lists all domains.
    pub async fn describe_domain_list(
        &self,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> Result<DescribeDomainListResponse, TencentError> {
        #[derive(Serialize)]
        struct Request {
            #[serde(rename = "Offset", skip_serializing_if = "Option::is_none")]
            offset: Option<u32>,
            #[serde(rename = "Limit", skip_serializing_if = "Option::is_none")]
            limit: Option<u32>,
        }

        self.request("DescribeDomainList", &Request { offset, limit })
            .await
    }

    /// Gets domain information by domain name.
    pub async fn describe_domain(
        &self,
        domain: &str,
    ) -> Result<DescribeDomainResponse, TencentError> {
        #[derive(Serialize)]
        struct Request<'a> {
            #[serde(rename = "Domain")]
            domain: &'a str,
        }

        self.request("DescribeDomain", &Request { domain }).await
    }

    /// Gets domain information by domain ID.
    pub async fn describe_domain_by_id(
        &self,
        domain_id: u64,
    ) -> Result<DescribeDomainResponse, TencentError> {
        #[derive(Serialize)]
        struct Request {
            #[serde(rename = "DomainId")]
            domain_id: u64,
        }

        self.request("DescribeDomain", &Request { domain_id }).await
    }

    /// Creates a new domain.
    pub async fn create_domain(&self, domain: &str) -> Result<CreateDomainResponse, TencentError> {
        #[derive(Serialize)]
        struct Request<'a> {
            #[serde(rename = "Domain")]
            domain: &'a str,
        }

        self.request("CreateDomain", &Request { domain }).await
    }

    /// Deletes a domain.
    pub async fn delete_domain(&self, domain: &str) -> Result<DeleteDomainResponse, TencentError> {
        #[derive(Serialize)]
        struct Request<'a> {
            #[serde(rename = "Domain")]
            domain: &'a str,
        }

        self.request("DeleteDomain", &Request { domain }).await
    }

    // =========================================================================
    // Record APIs
    // =========================================================================

    /// Lists all records for a domain.
    pub async fn describe_record_list(
        &self,
        domain: &str,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> Result<DescribeRecordListResponse, TencentError> {
        #[derive(Serialize)]
        struct Request<'a> {
            #[serde(rename = "Domain")]
            domain: &'a str,
            #[serde(rename = "Offset", skip_serializing_if = "Option::is_none")]
            offset: Option<u32>,
            #[serde(rename = "Limit", skip_serializing_if = "Option::is_none")]
            limit: Option<u32>,
        }

        self.request(
            "DescribeRecordList",
            &Request {
                domain,
                offset,
                limit,
            },
        )
        .await
    }

    /// Gets a record by ID.
    pub async fn describe_record(
        &self,
        domain: &str,
        record_id: u64,
    ) -> Result<DescribeRecordResponse, TencentError> {
        #[derive(Serialize)]
        struct Request<'a> {
            #[serde(rename = "Domain")]
            domain: &'a str,
            #[serde(rename = "RecordId")]
            record_id: u64,
        }

        self.request("DescribeRecord", &Request { domain, record_id })
            .await
    }

    /// Creates a new record.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_record(
        &self,
        domain: &str,
        sub_domain: &str,
        record_type: &str,
        record_line: &str,
        value: &str,
        mx: Option<u16>,
        ttl: Option<u64>,
    ) -> Result<CreateRecordResponse, TencentError> {
        #[derive(Serialize)]
        struct Request<'a> {
            #[serde(rename = "Domain")]
            domain: &'a str,
            #[serde(rename = "SubDomain")]
            sub_domain: &'a str,
            #[serde(rename = "RecordType")]
            record_type: &'a str,
            #[serde(rename = "RecordLine")]
            record_line: &'a str,
            #[serde(rename = "Value")]
            value: &'a str,
            #[serde(rename = "MX", skip_serializing_if = "Option::is_none")]
            mx: Option<u16>,
            #[serde(rename = "TTL", skip_serializing_if = "Option::is_none")]
            ttl: Option<u64>,
        }

        self.request(
            "CreateRecord",
            &Request {
                domain,
                sub_domain,
                record_type,
                record_line,
                value,
                mx,
                ttl,
            },
        )
        .await
    }

    /// Modifies an existing record.
    #[allow(clippy::too_many_arguments)]
    pub async fn modify_record(
        &self,
        domain: &str,
        record_id: u64,
        sub_domain: &str,
        record_type: &str,
        record_line: &str,
        value: &str,
        mx: Option<u16>,
        ttl: Option<u64>,
    ) -> Result<ModifyRecordResponse, TencentError> {
        #[derive(Serialize)]
        struct Request<'a> {
            #[serde(rename = "Domain")]
            domain: &'a str,
            #[serde(rename = "RecordId")]
            record_id: u64,
            #[serde(rename = "SubDomain")]
            sub_domain: &'a str,
            #[serde(rename = "RecordType")]
            record_type: &'a str,
            #[serde(rename = "RecordLine")]
            record_line: &'a str,
            #[serde(rename = "Value")]
            value: &'a str,
            #[serde(rename = "MX", skip_serializing_if = "Option::is_none")]
            mx: Option<u16>,
            #[serde(rename = "TTL", skip_serializing_if = "Option::is_none")]
            ttl: Option<u64>,
        }

        self.request(
            "ModifyRecord",
            &Request {
                domain,
                record_id,
                sub_domain,
                record_type,
                record_line,
                value,
                mx,
                ttl,
            },
        )
        .await
    }

    /// Deletes a record.
    pub async fn delete_record(
        &self,
        domain: &str,
        record_id: u64,
    ) -> Result<DeleteRecordResponse, TencentError> {
        #[derive(Serialize)]
        struct Request<'a> {
            #[serde(rename = "Domain")]
            domain: &'a str,
            #[serde(rename = "RecordId")]
            record_id: u64,
        }

        self.request("DeleteRecord", &Request { domain, record_id })
            .await
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

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

/// Parses a record value string into [`crate::RecordData`].
fn parse_record_data(
    record_type: &str,
    value: &str,
    mx: Option<u16>,
) -> Result<crate::RecordData, RecordConversionError> {
    use crate::RecordData;

    match record_type {
        "A" => value
            .parse()
            .map(RecordData::A)
            .map_err(|_| RecordConversionError {
                record_type: record_type.to_string(),
                reason: "invalid IPv4 address",
            }),
        "AAAA" => value
            .parse()
            .map(RecordData::AAAA)
            .map_err(|_| RecordConversionError {
                record_type: record_type.to_string(),
                reason: "invalid IPv6 address",
            }),
        "CNAME" => Ok(RecordData::CNAME(value.trim_end_matches('.').to_string())),
        "MX" => Ok(RecordData::MX {
            priority: mx.unwrap_or(10),
            mail_server: value.trim_end_matches('.').to_string(),
        }),
        "NS" => Ok(RecordData::NS(value.trim_end_matches('.').to_string())),
        "TXT" => Ok(RecordData::TXT(value.trim_matches('"').to_string())),
        "SRV" => {
            // SRV format: "priority weight port target"
            let parts: Vec<&str> = value.split_whitespace().collect();
            if parts.len() >= 4 {
                Ok(RecordData::SRV {
                    priority: parts[0].parse().map_err(|_| RecordConversionError {
                        record_type: record_type.to_string(),
                        reason: "invalid SRV priority",
                    })?,
                    weight: parts[1].parse().map_err(|_| RecordConversionError {
                        record_type: record_type.to_string(),
                        reason: "invalid SRV weight",
                    })?,
                    port: parts[2].parse().map_err(|_| RecordConversionError {
                        record_type: record_type.to_string(),
                        reason: "invalid SRV port",
                    })?,
                    target: parts[3].trim_end_matches('.').to_string(),
                })
            } else {
                Err(RecordConversionError {
                    record_type: record_type.to_string(),
                    reason: "SRV record requires 4 parts: priority weight port target",
                })
            }
        }
        _ => Ok(RecordData::Other {
            typ: record_type.to_string(),
            value: value.to_string(),
        }),
    }
}
