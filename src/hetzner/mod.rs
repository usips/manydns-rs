//! Hetzner Cloud DNS provider implementation.
//!
//! This provider uses the Hetzner Cloud API with Bearer token authentication.
//!
//! # Authentication
//!
//! Requires a Hetzner Cloud API token:
//! - Create a token at: <https://console.hetzner.cloud/projects/*/security/tokens>
//! - The token must have Read & Write permissions for DNS
//!
//! # Example
//!
//! ```no_run
//! use libdns::hetzner::HetznerProvider;
//! use libdns::{Provider, Zone, CreateZone, DeleteZone};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let provider = HetznerProvider::new("your_api_token")?;
//!
//! // List all zones
//! let zones = provider.list_zones().await?;
//! for zone in zones {
//!     println!("Zone: {} (ID: {})", zone.domain(), zone.id());
//! }
//!
//! // Create a new zone
//! let new_zone = provider.create_zone("example.com").await?;
//! println!("Created zone: {}", new_zone.domain());
//!
//! // Delete a zone
//! provider.delete_zone(new_zone.id()).await?;
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
//! - SOA (Start of authority)
//! - CAA (Certification Authority Authorization)
//! - TLSA (TLS Authentication)
//! - DS (Delegation Signer)
//! - RP (Responsible Person)
//! - HINFO (Host Information)
//! - PTR (Pointer record)
//! - HTTPS (HTTPS Service Binding)
//! - SVCB (Service Binding)
//!
//! # Zone Management
//!
//! Unlike some providers, Hetzner supports creating and deleting zones
//! through the API. See [`CreateZone`] and [`DeleteZone`] traits.
//!
//! # API Reference
//!
//! - [Hetzner Cloud API Documentation](https://docs.hetzner.cloud/)
//! - [DNS Zones](https://docs.hetzner.cloud/reference/cloud#zones)
//! - [DNS RRSets](https://docs.hetzner.cloud/reference/cloud#zone-rrsets)

pub mod api;

use std::error::Error as StdErr;
use std::sync::Arc;

use crate::{
    CreateRecord, CreateRecordError, CreateZone, CreateZoneError, DeleteRecord, DeleteRecordError,
    DeleteZone, DeleteZoneError, Provider, Record, RecordData, RetrieveRecordError,
    RetrieveZoneError, Zone,
};

/// Supported record types for Hetzner Cloud DNS.
const SUPPORTED_RECORD_TYPES: &[&str; 16] = &[
    "A", "AAAA", "NS", "MX", "CNAME", "RP", "TXT", "SOA", "HINFO", "SRV", "TLSA", "DS", "CAA",
    "PTR", "HTTPS", "SVCB",
];

/// Hetzner Cloud DNS provider.
///
/// Uses the Hetzner Cloud API with Bearer token authentication.
#[derive(Debug)]
pub struct HetznerProvider {
    api_client: Arc<api::Client>,
}

impl Clone for HetznerProvider {
    fn clone(&self) -> Self {
        HetznerProvider {
            api_client: Arc::clone(&self.api_client),
        }
    }
}

impl HetznerProvider {
    /// Creates a new Hetzner Cloud DNS provider.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Hetzner Cloud API token (Bearer token)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use libdns::hetzner::HetznerProvider;
    ///
    /// let provider = HetznerProvider::new("your_api_token").unwrap();
    /// ```
    pub fn new(api_key: &str) -> Result<Self, Box<dyn StdErr>> {
        let api_client = api::Client::new(api_key)?;
        Ok(Self {
            api_client: Arc::new(api_client),
        })
    }

    /// Creates a new Hetzner Cloud DNS provider with a custom API base URL.
    ///
    /// This is primarily useful for testing with mock servers.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Hetzner Cloud API token
    /// * `base_url` - Custom base URL for the API
    pub fn with_base_url(api_key: &str, base_url: &str) -> Result<Self, Box<dyn StdErr>> {
        let api_client = api::Client::with_base_url(api_key, base_url)?;
        Ok(Self {
            api_client: Arc::new(api_client),
        })
    }
}

impl Provider for HetznerProvider {
    type Zone = HetznerZone;
    type CustomRetrieveError = reqwest::Error;

    async fn get_zone(
        &self,
        zone_id: &str,
    ) -> Result<Self::Zone, RetrieveZoneError<Self::CustomRetrieveError>> {
        let response = self
            .api_client
            .retrieve_zone(zone_id)
            .await
            .map_err(|err| {
                if err.is_status() {
                    return match err.status().unwrap() {
                        reqwest::StatusCode::NOT_FOUND => RetrieveZoneError::NotFound,
                        reqwest::StatusCode::UNAUTHORIZED => RetrieveZoneError::Unauthorized,
                        _ => RetrieveZoneError::Custom(err),
                    };
                }
                RetrieveZoneError::Custom(err)
            })?;

        Ok(HetznerZone::from_api(
            self.api_client.clone(),
            response.zone,
        ))
    }

    async fn list_zones(
        &self,
    ) -> Result<Vec<Self::Zone>, RetrieveZoneError<Self::CustomRetrieveError>> {
        let mut zones = Vec::new();
        let mut total: Option<usize> = None;
        let mut page = 1;

        loop {
            let result =
                self.api_client
                    .retrieve_zones(page, 100)
                    .await
                    .map_err(|err| {
                        if err.is_status() {
                            return match err.status().unwrap() {
                                reqwest::StatusCode::NOT_FOUND => RetrieveZoneError::NotFound,
                                reqwest::StatusCode::UNAUTHORIZED
                                | reqwest::StatusCode::FORBIDDEN => RetrieveZoneError::Unauthorized,
                                _ => RetrieveZoneError::Custom(err),
                            };
                        }
                        RetrieveZoneError::Custom(err)
                    });

            match result {
                Ok(response) => {
                    if total.is_none() {
                        total = Some(response.meta.pagination.total_entries as usize);
                    }

                    zones.extend(
                        response
                            .zones
                            .into_iter()
                            .map(|zone| HetznerZone::from_api(self.api_client.clone(), zone)),
                    );
                }
                Err(err) => {
                    if let RetrieveZoneError::NotFound = err {
                        break;
                    }
                    return Err(err);
                }
            }

            if total.is_some_and(|t| zones.len() == t) {
                break;
            }

            page += 1;
        }

        Ok(zones)
    }
}

impl CreateZone for HetznerProvider {
    type CustomCreateError = reqwest::Error;

    async fn create_zone(
        &self,
        domain: &str,
    ) -> Result<Self::Zone, CreateZoneError<Self::CustomCreateError>> {
        let response = self
            .api_client
            .create_zone(domain, None)
            .await
            .map_err(|err| {
                if err.is_status() {
                    return match err.status().unwrap() {
                        reqwest::StatusCode::UNAUTHORIZED => CreateZoneError::Unauthorized,
                        reqwest::StatusCode::UNPROCESSABLE_ENTITY => {
                            CreateZoneError::InvalidDomainName
                        }
                        _ => CreateZoneError::Custom(err),
                    };
                }
                CreateZoneError::Custom(err)
            })?;

        Ok(HetznerZone::from_api(
            self.api_client.clone(),
            response.zone,
        ))
    }
}

impl DeleteZone for HetznerProvider {
    type CustomDeleteError = reqwest::Error;

    async fn delete_zone(
        &self,
        zone_id: &str,
    ) -> Result<(), DeleteZoneError<Self::CustomDeleteError>> {
        self.api_client.delete_zone(zone_id).await.map_err(|err| {
            if err.is_status() {
                return match err.status().unwrap() {
                    reqwest::StatusCode::NOT_FOUND => DeleteZoneError::NotFound,
                    reqwest::StatusCode::UNAUTHORIZED => DeleteZoneError::Unauthorized,
                    _ => DeleteZoneError::Custom(err),
                };
            }
            DeleteZoneError::Custom(err)
        })
    }
}

/// Represents a DNS zone in Hetzner Cloud DNS.
///
/// This struct provides methods for managing DNS records within a zone,
/// including listing, creating, and deleting records.
///
/// # Zone Status
///
/// Hetzner zones have a status that can be:
/// - `Ok` - The zone is active and DNS resolution works
/// - `Pending` - The zone is awaiting setup
/// - `Failed` - Zone setup failed
///
/// # RRSet-based API
///
/// The Hetzner Cloud API uses RRSets (Resource Record Sets) instead of
/// individual records. An RRSet is a group of records with the same name
/// and type. This implementation handles the conversion between the
/// libdns record model and Hetzner's RRSet model.
///
/// # Example
///
/// ```rust,no_run
/// use libdns::hetzner::HetznerProvider;
/// use libdns::{Provider, Zone, CreateRecord, DeleteRecord, RecordData};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let provider = HetznerProvider::new("your-api-token")?;
///
/// // Get an existing zone
/// let zone = provider.get_zone("example.com").await?;
/// println!("Zone ID: {}", zone.id());
/// println!("Domain: {}", zone.domain());
///
/// // List all records in the zone
/// let records = zone.list_records().await?;
/// for record in &records {
///     println!("{:?}", record);
/// }
///
/// // Create a new A record
/// let data = RecordData::A("1.2.3.4".parse()?);
/// zone.create_record("www", &data, 300).await?;
///
/// // Delete a record by ID (format: "name/type/value")
/// zone.delete_record("www/A/1.2.3.4").await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct HetznerZone {
    api_client: Arc<api::Client>,
    repr: api::Zone,
    /// Cached zone ID as string for the Zone trait.
    zone_id_str: String,
}

impl HetznerZone {
    /// Creates a new HetznerZone from API response data.
    fn from_api(api_client: Arc<api::Client>, zone: api::Zone) -> Self {
        let zone_id_str = zone.id.to_string();
        Self {
            api_client,
            repr: zone,
            zone_id_str,
        }
    }
}

impl Zone for HetznerZone {
    type CustomRetrieveError = reqwest::Error;

    fn id(&self) -> &str {
        &self.zone_id_str
    }

    fn domain(&self) -> &str {
        &self.repr.name
    }

    async fn list_records(
        &self,
    ) -> Result<Vec<Record>, RetrieveRecordError<Self::CustomRetrieveError>> {
        let mut records = Vec::new();
        let mut total: Option<usize> = None;
        let mut page = 1;

        loop {
            let result = self
                .api_client
                .retrieve_rrsets(&self.zone_id_str, page, 100)
                .await
                .map_err(|err| {
                    if err.is_status() {
                        return match err.status().unwrap() {
                            reqwest::StatusCode::NOT_FOUND => RetrieveRecordError::NotFound,
                            reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => {
                                RetrieveRecordError::Unauthorized
                            }
                            _ => RetrieveRecordError::Custom(err),
                        };
                    }
                    RetrieveRecordError::Custom(err)
                });

            match result {
                Ok(response) => {
                    if total.is_none() {
                        total = Some(response.meta.pagination.total_entries as usize);
                    }

                    let is_last_page =
                        response.meta.pagination.page >= response.meta.pagination.last_page;

                    // Convert RRSets to individual records
                    for rrset in &response.rrsets {
                        let ttl = rrset.ttl.unwrap_or(self.repr.ttl);
                        for record_value in &rrset.records {
                            // Create a unique ID: "name/type/value"
                            let record_id =
                                format!("{}/{}/{}", rrset.name, rrset.typ, record_value.value);
                            records.push(Record {
                                id: record_id,
                                host: rrset.name.clone(),
                                data: RecordData::from_raw(&rrset.typ, &record_value.value),
                                ttl,
                            });
                        }
                    }

                    if is_last_page {
                        break;
                    }
                }
                Err(err) => {
                    if let RetrieveRecordError::NotFound = err {
                        break;
                    }
                    return Err(err);
                }
            }

            page += 1;
        }

        Ok(records)
    }

    async fn get_record(
        &self,
        record_id: &str,
    ) -> Result<Record, RetrieveRecordError<Self::CustomRetrieveError>> {
        // Parse record ID format: "name/type/value"
        let parts: Vec<&str> = record_id.splitn(3, '/').collect();
        if parts.len() != 3 {
            return Err(RetrieveRecordError::NotFound);
        }
        let (name, typ, value) = (parts[0], parts[1], parts[2]);

        let response = self
            .api_client
            .retrieve_rrset(&self.zone_id_str, name, typ)
            .await
            .map_err(|err| {
                if err.is_status() {
                    return match err.status().unwrap() {
                        reqwest::StatusCode::NOT_FOUND => RetrieveRecordError::NotFound,
                        reqwest::StatusCode::UNAUTHORIZED => RetrieveRecordError::Unauthorized,
                        _ => RetrieveRecordError::Custom(err),
                    };
                }
                RetrieveRecordError::Custom(err)
            })?;

        // Find the specific record value in the RRSet
        let rrset = &response.rrset;
        let record_value = rrset
            .records
            .iter()
            .find(|r| r.value == value)
            .ok_or(RetrieveRecordError::NotFound)?;

        Ok(Record {
            id: record_id.to_string(),
            host: rrset.name.clone(),
            data: RecordData::from_raw(&rrset.typ, &record_value.value),
            ttl: rrset.ttl.unwrap_or(self.repr.ttl),
        })
    }
}

/// Format a record value for the Hetzner Cloud API.
///
/// TXT records must be wrapped in double quotes per Hetzner's requirements.
fn format_value_for_api(data: &RecordData) -> String {
    match data {
        RecordData::TXT(val) => {
            // Hetzner requires TXT values to be double-quoted
            if val.starts_with('"') && val.ends_with('"') {
                val.clone()
            } else {
                format!("\"{}\"", val)
            }
        }
        _ => data.get_value(),
    }
}

impl CreateRecord for HetznerZone {
    type CustomCreateError = reqwest::Error;

    async fn create_record(
        &self,
        host: &str,
        data: &RecordData,
        ttl: u64,
    ) -> Result<Record, CreateRecordError<Self::CustomCreateError>> {
        let typ = data.get_type();
        if !SUPPORTED_RECORD_TYPES.iter().any(|r| *r == typ) {
            return Err(CreateRecordError::UnsupportedType);
        }

        let value = format_value_for_api(data);
        let record_value = api::RecordValue::new(&value);

        let opt_ttl = if ttl != self.repr.ttl {
            Some(ttl)
        } else {
            None
        };

        // Try to add to existing RRSet first (this creates if it doesn't exist)
        let _response = self
            .api_client
            .add_records_to_rrset(&self.zone_id_str, host, typ, vec![record_value], opt_ttl)
            .await
            .map_err(|err| {
                if err.is_status() {
                    return match err.status().unwrap() {
                        reqwest::StatusCode::UNAUTHORIZED => CreateRecordError::Unauthorized,
                        reqwest::StatusCode::UNPROCESSABLE_ENTITY => {
                            CreateRecordError::InvalidRecord
                        }
                        _ => CreateRecordError::Custom(err),
                    };
                }
                CreateRecordError::Custom(err)
            })?;

        // Create the record ID in our format (use API value for consistency)
        let record_id = format!("{}/{}/{}", host, typ, value);

        Ok(Record {
            id: record_id,
            host: host.to_string(),
            data: data.clone(),
            ttl,
        })
    }
}

impl DeleteRecord for HetznerZone {
    type CustomDeleteError = reqwest::Error;

    async fn delete_record(
        &self,
        record_id: &str,
    ) -> Result<(), DeleteRecordError<Self::CustomDeleteError>> {
        // Parse record ID format: "name/type/value"
        let parts: Vec<&str> = record_id.splitn(3, '/').collect();
        if parts.len() != 3 {
            return Err(DeleteRecordError::NotFound);
        }
        let (name, typ, value) = (parts[0], parts[1], parts[2]);

        // Remove this specific record from the RRSet
        let record_value = api::RecordValue::new(value);

        self.api_client
            .remove_records_from_rrset(&self.zone_id_str, name, typ, vec![record_value])
            .await
            .map_err(|err| {
                if err.is_status() {
                    return match err.status().unwrap() {
                        reqwest::StatusCode::NOT_FOUND => DeleteRecordError::NotFound,
                        reqwest::StatusCode::UNAUTHORIZED => DeleteRecordError::Unauthorized,
                        _ => DeleteRecordError::Custom(err),
                    };
                }
                DeleteRecordError::Custom(err)
            })?;

        Ok(())
    }
}
