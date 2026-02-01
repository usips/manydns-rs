//! Technitium DNS Server provider implementation.
//!
//! This module provides a [`Provider`] implementation for [Technitium DNS Server](https://technitium.com/dns/).
//!
//! # Authentication
//!
//! The Technitium DNS API uses token-based authentication. You can obtain a token by:
//!
//! 1. **Session Token**: Login via the web interface or API - expires after 30 minutes of inactivity
//! 2. **API Token**: Create a non-expiring token via Settings -> API Token in the web interface
//!
//! For production use, API tokens are recommended as they don't expire.
//!
//! # Example
//!
//! ```no_run
//! use manydns::technitium::TechnitiumProvider;
//! use manydns::{Provider, Zone};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Using an API token (recommended)
//! let provider = TechnitiumProvider::new("http://localhost:5380", "your-api-token")?;
//!
//! // List all zones
//! let zones = provider.list_zones().await?;
//! for zone in zones {
//!     println!("Zone: {}", zone.domain());
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Zone IDs
//!
//! Unlike some other DNS providers, Technitium uses the zone's domain name as its identifier.
//! For example, to get the zone for `example.com`, use `"example.com"` as the zone ID.
//!
//! # Record IDs
//!
//! Technitium doesn't provide unique record IDs. Instead, records are identified by a combination
//! of domain name, record type, and record data. This implementation generates a composite ID
//! in the format `{domain}:{type}:{data_hash}` for compatibility with the generic Record interface.

use std::sync::Arc;

use crate::{
    CreateRecord, CreateRecordError, CreateZone, CreateZoneError, DeleteRecord, DeleteRecordError,
    DeleteZone, DeleteZoneError, HttpClientConfig, Provider, Record, RecordData,
    RetrieveRecordError, RetrieveZoneError, Zone,
};

pub mod api;

/// Supported record types for Technitium DNS.
const SUPPORTED_RECORD_TYPES: &[&str] = &[
    "A", "AAAA", "NS", "MX", "CNAME", "PTR", "TXT", "SRV", "DNAME", "DS", "SSHFP", "TLSA", "SVCB",
    "HTTPS", "URI", "CAA", "ANAME", "FWD", "APP",
];

/// Technitium DNS Server provider.
///
/// This provider implements DNS zone and record management for Technitium DNS Server.
/// It supports creating, listing, and deleting zones and records.
#[derive(Debug)]
pub struct TechnitiumProvider {
    api_client: Arc<api::Client>,
}

impl Clone for TechnitiumProvider {
    fn clone(&self) -> Self {
        TechnitiumProvider {
            api_client: Arc::new(self.api_client.as_ref().clone()),
        }
    }
}

impl TechnitiumProvider {
    /// Creates a new Technitium DNS provider with the given base URL and API token.
    ///
    /// # Arguments
    ///
    /// * `base_url` - The base URL of the Technitium DNS Server (e.g., `http://localhost:5380`)
    /// * `token` - The API token for authentication
    ///
    /// # Example
    ///
    /// ```no_run
    /// use manydns::technitium::TechnitiumProvider;
    ///
    /// let provider = TechnitiumProvider::new("http://localhost:5380", "my-api-token").unwrap();
    /// ```
    pub fn new(base_url: &str, token: &str) -> Result<Self, reqwest::Error> {
        let api_client = api::Client::new(base_url, token)?;
        Ok(Self {
            api_client: Arc::new(api_client),
        })
    }

    /// Creates a new Technitium DNS provider with custom HTTP configuration.
    ///
    /// # Arguments
    ///
    /// * `base_url` - The base URL of the Technitium DNS Server
    /// * `token` - The API token for authentication
    /// * `config` - HTTP client configuration for network binding
    ///
    /// # Example
    ///
    /// ```no_run
    /// use manydns::technitium::TechnitiumProvider;
    /// use manydns::HttpClientConfig;
    ///
    /// let config = HttpClientConfig::new()
    ///     .local_address("192.168.1.100".parse().unwrap());
    /// let provider = TechnitiumProvider::with_config("http://localhost:5380", "my-api-token", config).unwrap();
    /// ```
    pub fn with_config(
        base_url: &str,
        token: &str,
        config: HttpClientConfig,
    ) -> Result<Self, reqwest::Error> {
        let api_client = api::Client::with_config(base_url, token, config)?;
        Ok(Self {
            api_client: Arc::new(api_client),
        })
    }

    /// Creates a new Technitium DNS provider by logging in with username and password.
    ///
    /// This creates a session token that expires after 30 minutes of inactivity.
    /// For long-running applications, use [`TechnitiumProvider::new`] with an API token instead.
    ///
    /// # Arguments
    ///
    /// * `base_url` - The base URL of the Technitium DNS Server
    /// * `username` - The username (default is `admin`)
    /// * `password` - The password (default is `admin`)
    pub async fn login(
        base_url: &str,
        username: &str,
        password: &str,
    ) -> Result<Self, api::ApiError> {
        let api_client = api::Client::login(base_url, username, password).await?;
        Ok(Self {
            api_client: Arc::new(api_client),
        })
    }

    /// Creates a new Technitium DNS provider by logging in with custom HTTP configuration.
    ///
    /// This creates a session token that expires after 30 minutes of inactivity.
    /// For long-running applications, use [`TechnitiumProvider::with_config`] with an API token.
    ///
    /// # Arguments
    ///
    /// * `base_url` - The base URL of the Technitium DNS Server
    /// * `username` - The username (default is `admin`)
    /// * `password` - The password (default is `admin`)
    /// * `config` - HTTP client configuration for network binding
    ///
    /// # Example
    ///
    /// ```no_run
    /// use manydns::HttpClientConfig;
    /// use manydns::technitium::TechnitiumProvider;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = HttpClientConfig::new()
    ///     .local_address("192.168.1.100".parse().unwrap());
    /// let provider = TechnitiumProvider::login_with_config(
    ///     "http://localhost:5380",
    ///     "admin",
    ///     "password",
    ///     config,
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn login_with_config(
        base_url: &str,
        username: &str,
        password: &str,
        config: HttpClientConfig,
    ) -> Result<Self, api::ApiError> {
        let api_client =
            api::Client::login_with_config(base_url, username, password, config).await?;
        Ok(Self {
            api_client: Arc::new(api_client),
        })
    }
}

impl Provider for TechnitiumProvider {
    type Zone = TechnitiumZone;
    type CustomRetrieveError = api::ApiError;

    async fn get_zone(
        &self,
        zone_id: &str,
    ) -> Result<Self::Zone, RetrieveZoneError<Self::CustomRetrieveError>> {
        let response = self
            .api_client
            .get_zone(zone_id)
            .await
            .map_err(|err| match &err {
                api::ApiError::Unauthorized => RetrieveZoneError::Unauthorized,
                api::ApiError::NotFound => RetrieveZoneError::NotFound,
                _ => RetrieveZoneError::Custom(err),
            })?;

        Ok(TechnitiumZone {
            api_client: self.api_client.clone(),
            name: response.name,
            zone_type: response.zone_type,
            disabled: response.disabled,
        })
    }

    async fn list_zones(
        &self,
    ) -> Result<Vec<Self::Zone>, RetrieveZoneError<Self::CustomRetrieveError>> {
        let response = self
            .api_client
            .list_zones()
            .await
            .map_err(|err| match &err {
                api::ApiError::Unauthorized => RetrieveZoneError::Unauthorized,
                api::ApiError::NotFound => RetrieveZoneError::NotFound,
                _ => RetrieveZoneError::Custom(err),
            })?;

        Ok(response
            .zones
            .into_iter()
            .map(|zone| TechnitiumZone {
                api_client: self.api_client.clone(),
                name: zone.name,
                zone_type: zone.zone_type,
                disabled: zone.disabled,
            })
            .collect())
    }
}

impl CreateZone for TechnitiumProvider {
    type CustomCreateError = api::ApiError;

    async fn create_zone(
        &self,
        domain: &str,
    ) -> Result<Self::Zone, CreateZoneError<Self::CustomCreateError>> {
        let response = self
            .api_client
            .create_zone(domain)
            .await
            .map_err(|err| match &err {
                api::ApiError::Unauthorized => CreateZoneError::Unauthorized,
                api::ApiError::InvalidDomainName => CreateZoneError::InvalidDomainName,
                _ => CreateZoneError::Custom(err),
            })?;

        Ok(TechnitiumZone {
            api_client: self.api_client.clone(),
            name: response.domain,
            zone_type: "Primary".to_string(),
            disabled: false,
        })
    }
}

impl DeleteZone for TechnitiumProvider {
    type CustomDeleteError = api::ApiError;

    async fn delete_zone(
        &self,
        zone_id: &str,
    ) -> Result<(), DeleteZoneError<Self::CustomDeleteError>> {
        self.api_client
            .delete_zone(zone_id)
            .await
            .map_err(|err| match &err {
                api::ApiError::Unauthorized => DeleteZoneError::Unauthorized,
                api::ApiError::NotFound => DeleteZoneError::NotFound,
                _ => DeleteZoneError::Custom(err),
            })
    }
}

/// Represents a Technitium DNS zone.
#[derive(Debug, Clone)]
pub struct TechnitiumZone {
    api_client: Arc<api::Client>,
    name: String,
    zone_type: String,
    disabled: bool,
}

impl TechnitiumZone {
    /// Returns the zone type (Primary, Secondary, Stub, Forwarder, etc.)
    pub fn zone_type(&self) -> &str {
        &self.zone_type
    }

    /// Returns whether the zone is disabled.
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Enables the zone.
    pub async fn enable(&self) -> Result<(), api::ApiError> {
        self.api_client.enable_zone(&self.name).await
    }

    /// Disables the zone.
    pub async fn disable(&self) -> Result<(), api::ApiError> {
        self.api_client.disable_zone(&self.name).await
    }
}

impl Zone for TechnitiumZone {
    type CustomRetrieveError = api::ApiError;

    fn id(&self) -> &str {
        &self.name
    }

    fn domain(&self) -> &str {
        &self.name
    }

    async fn list_records(
        &self,
    ) -> Result<Vec<Record>, RetrieveRecordError<Self::CustomRetrieveError>> {
        let response =
            self.api_client
                .list_records(&self.name)
                .await
                .map_err(|err| match &err {
                    api::ApiError::Unauthorized => RetrieveRecordError::Unauthorized,
                    api::ApiError::NotFound => RetrieveRecordError::NotFound,
                    _ => RetrieveRecordError::Custom(err),
                })?;

        Ok(response.records.into_iter().map(Record::from).collect())
    }

    async fn get_record(
        &self,
        record_id: &str,
    ) -> Result<Record, RetrieveRecordError<Self::CustomRetrieveError>> {
        // Parse the composite record ID: "domain:type:data_hash"
        let parts: Vec<&str> = record_id.splitn(3, ':').collect();
        if parts.len() < 3 {
            return Err(RetrieveRecordError::NotFound);
        }

        let domain = parts[0];
        let record_type = parts[1];
        let data_hash = parts[2];

        let response = self
            .api_client
            .get_records(&self.name, domain)
            .await
            .map_err(|err| match &err {
                api::ApiError::Unauthorized => RetrieveRecordError::Unauthorized,
                api::ApiError::NotFound => RetrieveRecordError::NotFound,
                _ => RetrieveRecordError::Custom(err),
            })?;

        // Find the matching record
        response
            .records
            .into_iter()
            .map(Record::from)
            .find(|r| {
                r.data.get_type() == record_type
                    && format!("{:x}", calculate_hash(&r.data.get_value())) == data_hash
            })
            .ok_or(RetrieveRecordError::NotFound)
    }
}

impl CreateRecord for TechnitiumZone {
    type CustomCreateError = api::ApiError;

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

        let record_params = record_data_to_params(data);
        let domain = if host == "@" || host.is_empty() {
            self.name.clone()
        } else if host.ends_with('.') {
            host.trim_end_matches('.').to_string()
        } else {
            format!("{}.{}", host, self.name)
        };

        let response = self
            .api_client
            .add_record(&self.name, &domain, typ, ttl, &record_params)
            .await
            .map_err(|err| match &err {
                api::ApiError::Unauthorized => CreateRecordError::Unauthorized,
                api::ApiError::InvalidRecord => CreateRecordError::InvalidRecord,
                _ => CreateRecordError::Custom(err),
            })?;

        Ok(Record::from(response.added_record))
    }
}

impl DeleteRecord for TechnitiumZone {
    type CustomDeleteError = api::ApiError;

    async fn delete_record(
        &self,
        record_id: &str,
    ) -> Result<(), DeleteRecordError<Self::CustomDeleteError>> {
        // First, get the record to obtain its details
        let record = self.get_record(record_id).await.map_err(|err| match err {
            RetrieveRecordError::Unauthorized => DeleteRecordError::Unauthorized,
            RetrieveRecordError::NotFound => DeleteRecordError::NotFound,
            RetrieveRecordError::Custom(e) => DeleteRecordError::Custom(e),
        })?;

        let record_params = record_data_to_params(&record.data);

        self.api_client
            .delete_record(
                &self.name,
                &record.host,
                record.data.get_type(),
                &record_params,
            )
            .await
            .map_err(|err| match &err {
                api::ApiError::Unauthorized => DeleteRecordError::Unauthorized,
                api::ApiError::NotFound => DeleteRecordError::NotFound,
                _ => DeleteRecordError::Custom(err),
            })
    }
}

impl From<api::Record> for Record {
    fn from(record: api::Record) -> Self {
        let data = RecordData::from_raw(&record.record_type, &record.rdata.to_value_string());
        let data_hash = format!("{:x}", calculate_hash(&data.get_value()));

        Record {
            id: format!("{}:{}:{}", record.name, record.record_type, data_hash),
            host: record.name,
            data,
            ttl: record.ttl,
        }
    }
}

/// Converts a generic RecordData to API-specific RecordParams.
fn record_data_to_params(data: &RecordData) -> api::RecordParams {
    match data {
        RecordData::A(addr) => api::RecordParams::A {
            ip_address: addr.to_string(),
        },
        RecordData::AAAA(addr) => api::RecordParams::AAAA {
            ip_address: addr.to_string(),
        },
        RecordData::CNAME(cname) => api::RecordParams::CNAME {
            cname: cname.clone(),
        },
        RecordData::MX {
            priority,
            mail_server,
        } => api::RecordParams::MX {
            preference: *priority,
            exchange: mail_server.clone(),
        },
        RecordData::NS(ns) => api::RecordParams::NS {
            name_server: ns.clone(),
        },
        RecordData::TXT(txt) => api::RecordParams::TXT { text: txt.clone() },
        RecordData::SRV {
            priority,
            weight,
            port,
            target,
        } => api::RecordParams::SRV {
            priority: *priority,
            weight: *weight,
            port: *port,
            target: target.clone(),
        },
        RecordData::Other { value, .. } => api::RecordParams::Other {
            value: value.clone(),
        },
    }
}

/// Simple hash function for generating record IDs.
fn calculate_hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}
