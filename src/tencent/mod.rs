//! Tencent Cloud DNSPod provider implementation.
//!
//! This provider uses the Tencent Cloud API with TC3-HMAC-SHA256 signature authentication.
//! This is the modern API endpoint (`dnspod.intl.tencentcloudapi.com`) as opposed to
//! the legacy DNSPod API (`api.dnspod.com`).
//!
//! # Authentication
//!
//! Requires Tencent Cloud API credentials:
//! - `SecretId`: Your Tencent Cloud SecretId
//! - `SecretKey`: Your Tencent Cloud SecretKey
//!
//! Generate these at: <https://console.tencentcloud.com/capi>
//!
//! # Example
//!
//! ```no_run
//! use libdns::tencent::TencentProvider;
//! use libdns::{Provider, Zone};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! let provider = TencentProvider::new("your_secret_id", "your_secret_key")?;
//!
//! // List all zones
//! let zones = provider.list_zones().await?;
//! for zone in zones {
//!     println!("Zone: {} (ID: {})", zone.domain(), zone.id());
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Supported Record Types
//!
//! - A (IPv4 address)
//! - AAAA (IPv6 address)
//! - CNAME (Canonical name)
//! - MX (Mail exchange)
//! - NS (Name server)
//! - TXT (Text record)
//! - SRV (Service record)
//!
//! # API Reference
//!
//! - [API Category](https://www.tencentcloud.com/document/api/1157/49025)
//! - [Signature v3](https://www.tencentcloud.com/document/api/1157/49029)

pub mod api;

use std::error::Error as StdErr;
use std::sync::Arc;

pub use api::{ApiError, Client, TencentError};

use crate::{
    CreateRecord, CreateRecordError, CreateZone, CreateZoneError, DeleteRecord, DeleteRecordError,
    DeleteZone, DeleteZoneError, Provider, Record, RecordData, RetrieveRecordError,
    RetrieveZoneError, Zone,
};

/// Supported DNS record types for Tencent Cloud DNSPod.
const SUPPORTED_RECORD_TYPES: &[&str] = &["A", "AAAA", "CNAME", "MX", "NS", "TXT", "SRV"];

/// Tencent Cloud DNSPod provider.
///
/// Uses the Tencent Cloud API with TC3-HMAC-SHA256 signature authentication.
#[derive(Clone)]
pub struct TencentProvider {
    api_client: Arc<Client>,
}

/// A DNS zone managed by Tencent Cloud DNSPod.
pub struct TencentZone {
    api_client: Arc<Client>,
    /// The domain info.
    repr: api::DomainListItem,
}

impl TencentZone {
    /// Returns the domain name.
    pub fn domain(&self) -> &str {
        &self.repr.name
    }
}

impl TencentProvider {
    /// Creates a new Tencent Cloud DNSPod provider.
    ///
    /// # Arguments
    ///
    /// * `secret_id` - Tencent Cloud SecretId
    /// * `secret_key` - Tencent Cloud SecretKey
    ///
    /// # Example
    ///
    /// ```no_run
    /// use libdns::tencent::TencentProvider;
    ///
    /// let provider = TencentProvider::new("secret_id", "secret_key").unwrap();
    /// ```
    pub fn new(secret_id: &str, secret_key: &str) -> Result<Self, Box<dyn StdErr + Send + Sync>> {
        let api_client = Client::new(secret_id, secret_key)?;
        Ok(Self {
            api_client: Arc::new(api_client),
        })
    }
}

impl Provider for TencentProvider {
    type Zone = TencentZone;
    type CustomRetrieveError = TencentError;

    async fn get_zone(
        &self,
        zone_id: &str,
    ) -> Result<Self::Zone, RetrieveZoneError<Self::CustomRetrieveError>> {
        // zone_id can be either a domain ID (numeric) or domain name
        let response = if zone_id.chars().all(|c| c.is_ascii_digit()) {
            let domain_id: u64 = zone_id.parse().map_err(|_| RetrieveZoneError::NotFound)?;
            self.api_client.describe_domain_by_id(domain_id).await
        } else {
            self.api_client.describe_domain(zone_id).await
        };

        let domain_info = response.map_err(|err| match &err {
            TencentError::Api(api_err) => match api_err.code.as_str() {
                "AuthFailure" | "AuthFailure.SecretIdNotFound" | "AuthFailure.SignatureFailure" => {
                    RetrieveZoneError::Unauthorized
                }
                "InvalidParameterValue.DomainNotExists" | "ResourceNotFound.NoDataOfDomain" => {
                    RetrieveZoneError::NotFound
                }
                _ => RetrieveZoneError::Custom(err),
            },
            _ => RetrieveZoneError::Custom(err),
        })?;

        Ok(TencentZone {
            api_client: self.api_client.clone(),
            repr: api::DomainListItem {
                domain_id: domain_info.domain_info.domain_id,
                name: domain_info.domain_info.domain,
                status: domain_info.domain_info.status,
                ttl: domain_info.domain_info.ttl,
                record_count: domain_info.domain_info.record_count,
            },
        })
    }

    async fn list_zones(
        &self,
    ) -> Result<Vec<Self::Zone>, RetrieveZoneError<Self::CustomRetrieveError>> {
        let mut zones = Vec::new();
        let mut offset: u32 = 0;
        const LIMIT: u32 = 500;

        loop {
            let response = self
                .api_client
                .describe_domain_list(Some(offset), Some(LIMIT))
                .await
                .map_err(|err| match &err {
                    TencentError::Api(api_err) => match api_err.code.as_str() {
                        "AuthFailure"
                        | "AuthFailure.SecretIdNotFound"
                        | "AuthFailure.SignatureFailure" => RetrieveZoneError::Unauthorized,
                        _ => RetrieveZoneError::Custom(err),
                    },
                    _ => RetrieveZoneError::Custom(err),
                })?;

            let domains = response.domain_list.unwrap_or_default();
            let domain_count = domains.len();

            zones.extend(domains.into_iter().map(|domain| TencentZone {
                api_client: self.api_client.clone(),
                repr: domain,
            }));

            if domain_count < LIMIT as usize {
                break;
            }

            offset += LIMIT;
        }

        Ok(zones)
    }
}

impl CreateZone for TencentProvider {
    type CustomCreateError = TencentError;

    async fn create_zone(
        &self,
        domain: &str,
    ) -> Result<Self::Zone, CreateZoneError<Self::CustomCreateError>> {
        let response = self
            .api_client
            .create_domain(domain)
            .await
            .map_err(|err| match &err {
                TencentError::Api(api_err) => match api_err.code.as_str() {
                    "AuthFailure"
                    | "AuthFailure.SecretIdNotFound"
                    | "AuthFailure.SignatureFailure" => CreateZoneError::Unauthorized,
                    "InvalidParameter.DomainInvalid" => CreateZoneError::InvalidDomainName,
                    _ => CreateZoneError::Custom(err),
                },
                _ => CreateZoneError::Custom(err),
            })?;

        // Fetch the full domain info
        let domain_info = self
            .api_client
            .describe_domain_by_id(response.domain_info.id)
            .await
            .map_err(CreateZoneError::Custom)?;

        Ok(TencentZone {
            api_client: self.api_client.clone(),
            repr: api::DomainListItem {
                domain_id: domain_info.domain_info.domain_id,
                name: domain_info.domain_info.domain,
                status: domain_info.domain_info.status,
                ttl: domain_info.domain_info.ttl,
                record_count: domain_info.domain_info.record_count,
            },
        })
    }
}

impl DeleteZone for TencentProvider {
    type CustomDeleteError = TencentError;

    async fn delete_zone(
        &self,
        zone_id: &str,
    ) -> Result<(), DeleteZoneError<Self::CustomDeleteError>> {
        // zone_id should be the domain name for Tencent API
        self.api_client
            .delete_domain(zone_id)
            .await
            .map_err(|err| match &err {
                TencentError::Api(api_err) => match api_err.code.as_str() {
                    "AuthFailure"
                    | "AuthFailure.SecretIdNotFound"
                    | "AuthFailure.SignatureFailure" => DeleteZoneError::Unauthorized,
                    "InvalidParameterValue.DomainNotExists" | "ResourceNotFound.NoDataOfDomain" => {
                        DeleteZoneError::NotFound
                    }
                    _ => DeleteZoneError::Custom(err),
                },
                _ => DeleteZoneError::Custom(err),
            })?;

        Ok(())
    }
}

impl Zone for TencentZone {
    type CustomRetrieveError = TencentError;

    fn id(&self) -> &str {
        // We store the domain name as the ID since Tencent API uses domain names
        &self.repr.name
    }

    fn domain(&self) -> &str {
        &self.repr.name
    }

    async fn list_records(
        &self,
    ) -> Result<Vec<Record>, RetrieveRecordError<Self::CustomRetrieveError>> {
        let mut records = Vec::new();
        let mut offset: u32 = 0;
        const LIMIT: u32 = 500;

        loop {
            let response = self
                .api_client
                .describe_record_list(&self.repr.name, Some(offset), Some(LIMIT))
                .await
                .map_err(|err| match &err {
                    TencentError::Api(api_err) => match api_err.code.as_str() {
                        "AuthFailure"
                        | "AuthFailure.SecretIdNotFound"
                        | "AuthFailure.SignatureFailure" => RetrieveRecordError::Unauthorized,
                        _ => RetrieveRecordError::Custom(err),
                    },
                    _ => RetrieveRecordError::Custom(err),
                })?;

            let record_list = response.record_list.unwrap_or_default();
            let record_count = record_list.len();

            for record in record_list {
                if let Some(data) = parse_record_data(&record.record_type, &record.value, record.mx)
                {
                    records.push(Record {
                        id: record.record_id.to_string(),
                        host: record.name.clone(),
                        data,
                        ttl: record.ttl,
                    });
                }
            }

            if record_count < LIMIT as usize {
                break;
            }

            offset += LIMIT;
        }

        Ok(records)
    }

    async fn get_record(
        &self,
        record_id: &str,
    ) -> Result<Record, RetrieveRecordError<Self::CustomRetrieveError>> {
        let record_id_num: u64 = record_id
            .parse()
            .map_err(|_| RetrieveRecordError::NotFound)?;

        let response = self
            .api_client
            .describe_record(&self.repr.name, record_id_num)
            .await
            .map_err(|err| match &err {
                TencentError::Api(api_err) => match api_err.code.as_str() {
                    "AuthFailure"
                    | "AuthFailure.SecretIdNotFound"
                    | "AuthFailure.SignatureFailure" => RetrieveRecordError::Unauthorized,
                    "InvalidParameter.RecordIdInvalid" | "ResourceNotFound.NoDataOfRecord" => {
                        RetrieveRecordError::NotFound
                    }
                    _ => RetrieveRecordError::Custom(err),
                },
                _ => RetrieveRecordError::Custom(err),
            })?;

        let record_info = &response.record_info;
        let data = parse_record_data(&record_info.record_type, &record_info.value, record_info.mx)
            .ok_or(RetrieveRecordError::NotFound)?;

        Ok(Record {
            id: record_info.id.to_string(),
            host: record_info.sub_domain.clone(),
            data,
            ttl: record_info.ttl,
        })
    }
}

impl CreateRecord for TencentZone {
    type CustomCreateError = TencentError;

    async fn create_record(
        &self,
        host: &str,
        data: &RecordData,
        ttl: u64,
    ) -> Result<Record, CreateRecordError<Self::CustomCreateError>> {
        let typ = data.get_type();
        if !SUPPORTED_RECORD_TYPES.contains(&typ) {
            return Err(CreateRecordError::UnsupportedType);
        }

        let mx = match data {
            RecordData::MX { priority, .. } => Some(*priority),
            _ => None,
        };

        let value = get_record_value(data);

        let response = self
            .api_client
            .create_record(
                &self.repr.name,
                host,
                typ,
                "默认", // Default line for Tencent Cloud
                &value,
                mx,
                Some(ttl),
            )
            .await
            .map_err(|err| match &err {
                TencentError::Api(api_err) => match api_err.code.as_str() {
                    "AuthFailure"
                    | "AuthFailure.SecretIdNotFound"
                    | "AuthFailure.SignatureFailure" => CreateRecordError::Unauthorized,
                    "InvalidParameter.RecordTypeInvalid" => CreateRecordError::UnsupportedType,
                    "InvalidParameter.SubDomainInvalid"
                    | "InvalidParameter.RecordValueInvalid"
                    | "InvalidParameter.MXInvalid" => CreateRecordError::InvalidRecord,
                    _ => CreateRecordError::Custom(err),
                },
                _ => CreateRecordError::Custom(err),
            })?;

        Ok(Record {
            id: response.record_id.to_string(),
            host: host.to_string(),
            data: data.clone(),
            ttl,
        })
    }
}

impl DeleteRecord for TencentZone {
    type CustomDeleteError = TencentError;

    async fn delete_record(
        &self,
        record_id: &str,
    ) -> Result<(), DeleteRecordError<Self::CustomDeleteError>> {
        let record_id_num: u64 = record_id.parse().map_err(|_| DeleteRecordError::NotFound)?;

        self.api_client
            .delete_record(&self.repr.name, record_id_num)
            .await
            .map_err(|err| match &err {
                TencentError::Api(api_err) => match api_err.code.as_str() {
                    "AuthFailure"
                    | "AuthFailure.SecretIdNotFound"
                    | "AuthFailure.SignatureFailure" => DeleteRecordError::Unauthorized,
                    "InvalidParameter.RecordIdInvalid" | "ResourceNotFound.NoDataOfRecord" => {
                        DeleteRecordError::NotFound
                    }
                    _ => DeleteRecordError::Custom(err),
                },
                _ => DeleteRecordError::Custom(err),
            })?;

        Ok(())
    }
}

/// Parses a record value string into RecordData.
fn parse_record_data(record_type: &str, value: &str, mx: Option<u16>) -> Option<RecordData> {
    match record_type {
        "A" => value.parse().ok().map(RecordData::A),
        "AAAA" => value.parse().ok().map(RecordData::AAAA),
        "CNAME" => Some(RecordData::CNAME(value.trim_end_matches('.').to_string())),
        "MX" => Some(RecordData::MX {
            priority: mx.unwrap_or(10),
            mail_server: value.trim_end_matches('.').to_string(),
        }),
        "NS" => Some(RecordData::NS(value.trim_end_matches('.').to_string())),
        "TXT" => Some(RecordData::TXT(value.trim_matches('"').to_string())),
        "SRV" => {
            // SRV format: "priority weight port target"
            let parts: Vec<&str> = value.split_whitespace().collect();
            if parts.len() >= 4 {
                Some(RecordData::SRV {
                    priority: parts[0].parse().ok()?,
                    weight: parts[1].parse().ok()?,
                    port: parts[2].parse().ok()?,
                    target: parts[3].trim_end_matches('.').to_string(),
                })
            } else {
                None
            }
        }
        _ => Some(RecordData::Other {
            typ: record_type.to_string(),
            value: value.to_string(),
        }),
    }
}

/// Gets the value string for a record, excluding MX priority (which is sent separately).
fn get_record_value(data: &RecordData) -> String {
    match data {
        RecordData::MX { mail_server, .. } => mail_server.clone(),
        RecordData::SRV {
            priority,
            weight,
            port,
            target,
        } => format!("{} {} {} {}", priority, weight, port, target),
        _ => data.get_value(),
    }
}
