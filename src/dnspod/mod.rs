//! DNSPod DNS provider implementation.
//!
//! This module provides an implementation of the libdns traits for DNSPod, a DNS service by Tencent.
//!
//! # Authentication
//!
//! DNSPod uses API tokens for authentication. You can generate a token from the DNSPod console.
//! The token format is `{SecretID},{SecretKey}`.
//!
//! # User-Agent Requirement
//!
//! DNSPod API mandates a properly formatted User-Agent header that identifies **your application**
//! (not this library) and includes a contact email address. This is enforced by Tencent and
//! non-compliance will result in your account being banned from API access.
//!
//! The format is: `ProgramName/Version (contact@email.com)`
//!
//! See: <https://docs.dnspod.com/api/api-development/>
//!
//! # Example
//!
//! ```no_run
//! use libdns::dnspod::{DnspodProvider, ClientConfig};
//! use libdns::{Provider, Zone};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Configure with YOUR program name and contact email (not the library's)
//! let config = ClientConfig::new("My DDNS App", "1.0.0", "developer@example.com");
//! let provider = DnspodProvider::new("your_secret_id,your_secret_key", &config)?;
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
//! DNSPod supports the following record types:
//! - A, AAAA, CNAME, MX, TXT, NS, SRV, URL, Framed URL
//!
//! # API Documentation
//!
//! For more information, see the [DNSPod API documentation](https://docs.dnspod.com/api/).

use std::{error::Error as StdErr, sync::Arc};

use crate::{
    CreateRecord, CreateRecordError, CreateZone, CreateZoneError, DeleteRecord, DeleteRecordError,
    DeleteZone, DeleteZoneError, Provider, Record, RecordData, RetrieveRecordError,
    RetrieveZoneError, Zone,
};

pub mod api;

pub use api::{ClientConfig, DnspodError, RecordInfoWithTtl, RecordWithTtl};

const SUPPORTED_RECORD_TYPES: &[&str; 9] =
    &["A", "AAAA", "CNAME", "MX", "TXT", "NS", "SRV", "URL", "CAA"];

/// DNSPod DNS provider.
///
/// Implements zone and record management for DNSPod.
#[derive(Debug)]
pub struct DnspodProvider {
    api_client: Arc<api::Client>,
}

impl Clone for DnspodProvider {
    fn clone(&self) -> Self {
        DnspodProvider {
            api_client: Arc::from(self.api_client.as_ref().clone()),
        }
    }
}

impl DnspodProvider {
    /// Creates a new DNSPod provider with the given API token and configuration.
    ///
    /// # Arguments
    ///
    /// * `login_token` - The DNSPod API token in format `{SecretID},{SecretKey}`
    /// * `config` - Client configuration with User-Agent details (program name, version, email)
    ///
    /// # User-Agent Requirement
    ///
    /// DNSPod API requires a User-Agent header that identifies your application
    /// (not this library) and provides a contact email. See [`ClientConfig`].
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use libdns::dnspod::{DnspodProvider, ClientConfig};
    ///
    /// let config = ClientConfig::new("My DDNS App", "1.0.0", "me@example.com");
    /// let provider = DnspodProvider::new("secret_id,secret_key", &config).unwrap();
    /// ```
    pub fn new(login_token: &str, config: &api::ClientConfig) -> Result<Self, Box<dyn StdErr>> {
        let api_client = api::Client::new(login_token, config)?;
        Ok(Self {
            api_client: Arc::new(api_client),
        })
    }
}

impl Provider for DnspodProvider {
    type Zone = DnspodZone;
    type CustomRetrieveError = DnspodError;

    async fn get_zone(
        &self,
        zone_id: &str,
    ) -> Result<Self::Zone, RetrieveZoneError<Self::CustomRetrieveError>> {
        // Try by domain name first if zone_id looks like a domain (contains a dot)
        // Otherwise try by numeric ID
        let response = if zone_id.contains('.') {
            self.api_client.get_domain_by_name(zone_id).await
        } else {
            self.api_client.get_domain(zone_id).await
        }
        .map_err(|err| match &err {
            DnspodError::Api(status) => {
                // DNSPod error codes
                match status.code.as_str() {
                    "-1" => RetrieveZoneError::Unauthorized,
                    "6" | "8" => RetrieveZoneError::NotFound, // Invalid domain id or no permission
                    _ => RetrieveZoneError::Custom(err),
                }
            }
            DnspodError::Request(_) => RetrieveZoneError::Custom(err),
        })?;

        // domain is guaranteed to be Some after successful API call
        Ok(DnspodZone {
            api_client: self.api_client.clone(),
            repr: response.domain.expect("domain should be present after success check"),
        })
    }

    async fn list_zones(
        &self,
    ) -> Result<Vec<Self::Zone>, RetrieveZoneError<Self::CustomRetrieveError>> {
        let mut zones = Vec::new();
        let mut offset: u32 = 0;
        const PAGE_SIZE: u32 = 500;

        loop {
            let result = self
                .api_client
                .list_domains(Some(offset), Some(PAGE_SIZE))
                .await
                .map_err(|err| match &err {
                    DnspodError::Api(status) => match status.code.as_str() {
                        "-1" => RetrieveZoneError::Unauthorized,
                        "9" => RetrieveZoneError::NotFound, // Empty result
                        _ => RetrieveZoneError::Custom(err),
                    },
                    DnspodError::Request(_) => RetrieveZoneError::Custom(err),
                });

            match result {
                Ok(response) => {
                    let domains = response.domains.unwrap_or_default();
                    let domain_count = domains.len();

                    zones.extend(domains.into_iter().map(|domain| DnspodZone {
                        api_client: self.api_client.clone(),
                        repr: domain,
                    }));

                    // Check if we've retrieved all domains
                    if domain_count < PAGE_SIZE as usize {
                        break;
                    }

                    offset += PAGE_SIZE;
                }
                Err(RetrieveZoneError::NotFound) => {
                    // Empty result, we're done
                    break;
                }
                Err(err) => return Err(err),
            }
        }

        Ok(zones)
    }
}

impl CreateZone for DnspodProvider {
    type CustomCreateError = DnspodError;

    async fn create_zone(
        &self,
        domain: &str,
    ) -> Result<Self::Zone, CreateZoneError<Self::CustomCreateError>> {
        let create_response =
            self.api_client
                .create_domain(domain)
                .await
                .map_err(|err| match &err {
                    DnspodError::Api(status) => match status.code.as_str() {
                        "-1" => CreateZoneError::Unauthorized,
                        "6" => CreateZoneError::InvalidDomainName, // Invalid domain
                        "7" => CreateZoneError::InvalidDomainName, // Domain already exists
                        "11" => CreateZoneError::InvalidDomainName, // Domain exists as alias
                        "12" => CreateZoneError::Unauthorized,     // No permission
                        "41" => CreateZoneError::InvalidDomainName, // Terms of service
                        _ => CreateZoneError::Custom(err),
                    },
                    DnspodError::Request(_) => CreateZoneError::Custom(err),
                })?;

        // Fetch the full domain info
        let domain_response = self
            .api_client
            .get_domain(&create_response.domain.id)
            .await
            .map_err(CreateZoneError::Custom)?;

        Ok(DnspodZone {
            api_client: self.api_client.clone(),
            repr: domain_response
                .domain
                .expect("domain should be present after success check"),
        })
    }
}

impl DeleteZone for DnspodProvider {
    type CustomDeleteError = DnspodError;

    async fn delete_zone(
        &self,
        zone_id: &str,
    ) -> Result<(), DeleteZoneError<Self::CustomDeleteError>> {
        self.api_client
            .delete_domain(zone_id)
            .await
            .map_err(|err| match &err {
                DnspodError::Api(status) => match status.code.as_str() {
                    "-1" => DeleteZoneError::Unauthorized,
                    "-15" => DeleteZoneError::Unauthorized, // Domain prohibited
                    "6" => DeleteZoneError::NotFound,       // Invalid domain id
                    "7" => DeleteZoneError::Unauthorized,   // Domain locked
                    "8" => DeleteZoneError::Unauthorized,   // VIP domain
                    "9" => DeleteZoneError::Unauthorized,   // No permission
                    _ => DeleteZoneError::Custom(err),
                },
                DnspodError::Request(_) => DeleteZoneError::Custom(err),
            })?;

        Ok(())
    }
}

/// Represents a DNSPod DNS zone.
#[derive(Debug, Clone)]
pub struct DnspodZone {
    api_client: Arc<api::Client>,
    repr: api::Domain,
}

impl Zone for DnspodZone {
    type CustomRetrieveError = DnspodError;

    fn id(&self) -> &str {
        &self.repr.id
    }

    fn domain(&self) -> &str {
        &self.repr.name
    }

    async fn list_records(
        &self,
    ) -> Result<Vec<Record>, RetrieveRecordError<Self::CustomRetrieveError>> {
        let mut records = Vec::new();
        let mut offset: u32 = 0;
        const PAGE_SIZE: u32 = 500;
        let default_ttl = self.repr.get_ttl();

        loop {
            let result = self
                .api_client
                .list_records(&self.repr.id, Some(offset), Some(PAGE_SIZE))
                .await
                .map_err(|err| match &err {
                    DnspodError::Api(status) => match status.code.as_str() {
                        "-1" => RetrieveRecordError::Unauthorized,
                        "6" => RetrieveRecordError::NotFound, // Invalid domain id
                        "9" => RetrieveRecordError::Unauthorized, // No permission
                        "10" => RetrieveRecordError::NotFound, // Empty result (handled below)
                        _ => RetrieveRecordError::Custom(err),
                    },
                    DnspodError::Request(_) => RetrieveRecordError::Custom(err),
                });

            match result {
                Ok(response) => {
                    let api_records = response.records.unwrap_or_default();
                    let record_count = api_records.len();

                    records.extend(
                        api_records
                            .iter()
                            .map(|r| crate::Record::from(api::RecordWithTtl::new(r, default_ttl))),
                    );

                    // Check if we've retrieved all records
                    if record_count < PAGE_SIZE as usize {
                        break;
                    }

                    offset += PAGE_SIZE;
                }
                Err(RetrieveRecordError::NotFound) => {
                    // Empty result, we're done
                    break;
                }
                Err(err) => return Err(err),
            }
        }

        Ok(records)
    }

    async fn get_record(
        &self,
        record_id: &str,
    ) -> Result<Record, RetrieveRecordError<Self::CustomRetrieveError>> {
        let default_ttl = self.repr.get_ttl();

        let response = self
            .api_client
            .get_record(&self.repr.id, record_id)
            .await
            .map_err(|err| match &err {
                DnspodError::Api(status) => match status.code.as_str() {
                    "-1" => RetrieveRecordError::Unauthorized,
                    "6" => RetrieveRecordError::NotFound, // Invalid domain id
                    "7" => RetrieveRecordError::Unauthorized, // No permission
                    "8" => RetrieveRecordError::NotFound, // Invalid record id
                    _ => RetrieveRecordError::Custom(err),
                },
                DnspodError::Request(_) => RetrieveRecordError::Custom(err),
            })?;

        Ok(crate::Record::from(api::RecordInfoWithTtl::new(
            &response.record,
            default_ttl,
        )))
    }
}

impl CreateRecord for DnspodZone {
    type CustomCreateError = DnspodError;

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

        // Parse MX priority if needed
        let mx = match data {
            RecordData::MX { priority, .. } => Some(*priority),
            _ => None,
        };

        // Get the record value
        let value = data.get_api_value();

        let response = self
            .api_client
            .create_record(
                &self.repr.id,
                host,
                typ,
                "default", // DNSPod uses "default" as the default record line
                &value,
                mx,
                Some(ttl),
            )
            .await
            .map_err(|err| match &err {
                DnspodError::Api(status) => match status.code.as_str() {
                    "-1" => CreateRecordError::Unauthorized,
                    "-15" => CreateRecordError::Unauthorized, // Domain prohibited
                    "6" => CreateRecordError::InvalidRecord,  // Lack of parameters
                    "7" => CreateRecordError::Unauthorized,   // No permission
                    "21" => CreateRecordError::Unauthorized,  // Domain locked
                    "22" | "23" | "24" | "25" => CreateRecordError::InvalidRecord, // Invalid subdomain
                    "26" => CreateRecordError::InvalidRecord,                      // Invalid line
                    "27" => CreateRecordError::UnsupportedType, // Invalid record type
                    "30" => CreateRecordError::InvalidRecord,   // Invalid MX
                    "31" | "32" | "33" => CreateRecordError::InvalidRecord, // Limit reached
                    "34" => CreateRecordError::InvalidRecord,   // Invalid record value
                    _ => CreateRecordError::Custom(err),
                },
                DnspodError::Request(_) => CreateRecordError::Custom(err),
            })?;

        // Get the record from response (should always be present if status was successful)
        let record_data = response.record.ok_or_else(|| {
            CreateRecordError::Custom(DnspodError::Api(api::Status {
                code: "0".to_string(),
                message: "API returned success but no record data".to_string(),
                created_at: None,
            }))
        })?;

        // Return a generic record with the created ID
        Ok(Record {
            id: record_data.id,
            host: host.to_string(),
            data: data.clone(),
            ttl,
        })
    }
}

impl DeleteRecord for DnspodZone {
    type CustomDeleteError = DnspodError;

    async fn delete_record(
        &self,
        record_id: &str,
    ) -> Result<(), DeleteRecordError<Self::CustomDeleteError>> {
        self.api_client
            .delete_record(&self.repr.id, record_id)
            .await
            .map_err(|err| match &err {
                DnspodError::Api(status) => match status.code.as_str() {
                    "-1" => DeleteRecordError::Unauthorized,
                    "-15" => DeleteRecordError::Unauthorized, // Domain prohibited
                    "6" => DeleteRecordError::NotFound,       // Invalid domain id
                    "7" => DeleteRecordError::Unauthorized,   // No permission
                    "8" => DeleteRecordError::NotFound,       // Invalid record id
                    "21" => DeleteRecordError::Unauthorized,  // Domain locked
                    _ => DeleteRecordError::Custom(err),
                },
                DnspodError::Request(_) => DeleteRecordError::Custom(err),
            })?;

        Ok(())
    }
}
