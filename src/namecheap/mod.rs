//! Namecheap DNS provider implementation.
//!
//! This provider uses the Namecheap API for DNS record management.
//!
//! # Important Notes
//!
//! - **IP Whitelisting Required**: You must whitelist your client IP in the Namecheap dashboard
//!   before API calls will work.
//! - **Destructive Updates**: The `setHosts` API replaces ALL records. This provider handles
//!   this by fetching existing records before modifications.
//! - **Zone ID Format**: Use the domain name as the zone ID (e.g., "example.com").
//!
//! # Environments
//!
//! Namecheap provides both sandbox and production APIs:
//! - Sandbox: `https://api.sandbox.namecheap.com/xml.response` (for testing)
//! - Production: `https://api.namecheap.com/xml.response`
//!
//! # Authentication
//!
//! Requires:
//! - API username (your Namecheap account username)
//! - API key (generated in Namecheap dashboard)
//! - Whitelisted client IP address
//!
//! # Example
//!
//! ```no_run
//! use manydns::namecheap::{NamecheapProvider, ClientConfig};
//! use manydns::types::Environment;
//! use manydns::{Provider, Zone};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! // Configure for sandbox testing
//! let config = ClientConfig::sandbox("your_username", "your_api_key", "your_whitelisted_ip");
//! let provider = NamecheapProvider::new(config)?;
//!
//! // Get a zone by domain name
//! let zone = provider.get_zone("example.com").await?;
//!
//! // List all records
//! let records = zone.list_records().await?;
//! for record in records {
//!     println!("{}: {} -> {:?}", record.host, record.data.get_type(), record.data.get_value());
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
//! - URL, URL301 (URL redirects)
//! - ALIAS, CAA, FRAME
//!
//! # API Reference
//!
//! - [API Introduction](https://www.namecheap.com/support/api/intro/)
//! - [getHosts](https://www.namecheap.com/support/api/methods/domains-dns/get-hosts/)
//! - [setHosts](https://www.namecheap.com/support/api/methods/domains-dns/set-hosts/)

pub mod api;

use std::error::Error as StdErr;
use std::sync::Arc;

pub use api::{
    get_element_attr, parse_host_records, ApiError, Client, ClientConfig, HostRecord,
    NamecheapError,
};

use crate::{
    CreateRecord, CreateRecordError, DeleteRecord, DeleteRecordError, HttpClientConfig, Provider,
    Record, RecordData, RetrieveRecordError, RetrieveZoneError, Zone,
};

/// Namecheap DNS provider.
///
/// Manages DNS records through the Namecheap API.
#[derive(Clone)]
pub struct NamecheapProvider {
    api_client: Arc<Client>,
}

/// A DNS zone managed by Namecheap.
///
/// The zone represents a domain (e.g., "example.com") and provides
/// methods to manage its DNS records.
pub struct NamecheapZone {
    api_client: Arc<Client>,
    /// The domain name (e.g., "example.com").
    domain: String,
    /// Second-level domain (e.g., "example").
    sld: String,
    /// Top-level domain (e.g., "com").
    tld: String,
}

impl NamecheapZone {
    /// Returns the domain name.
    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Returns the SLD (second-level domain).
    pub fn sld(&self) -> &str {
        &self.sld
    }

    /// Returns the TLD (top-level domain).
    pub fn tld(&self) -> &str {
        &self.tld
    }

    /// Fetches all current host records from the API.
    pub async fn fetch_records(&self) -> Result<Vec<HostRecord>, NamecheapError> {
        self.api_client.get_hosts(&self.sld, &self.tld).await
    }

    /// Saves all host records to the API, replacing all existing records.
    ///
    /// **Important**: This replaces ALL records in the zone. Include all records
    /// you want to keep (MX, TXT, CNAME, etc.) alongside any changes.
    pub async fn save_records(&self, records: &[HostRecord]) -> Result<(), NamecheapError> {
        self.api_client
            .set_hosts(&self.sld, &self.tld, records)
            .await
    }
}

/// Splits a domain into SLD and TLD parts.
///
/// For "example.com", returns ("example", "com").
/// For "sub.example.co.uk", returns ("sub.example", "co.uk") - handles common ccTLDs.
///
/// This is useful for working with the Namecheap API which requires separate SLD/TLD parameters.
pub fn split_domain(domain: &str) -> Option<(String, String)> {
    let domain = domain.trim_end_matches('.');
    let parts: Vec<&str> = domain.split('.').collect();

    if parts.len() < 2 {
        return None;
    }

    // Handle common two-part TLDs
    let two_part_tlds = [
        "co.uk",
        "org.uk",
        "me.uk",
        "net.uk",
        "ac.uk",
        "gov.uk",
        "ltd.uk",
        "plc.uk",
        "com.au",
        "net.au",
        "org.au",
        "edu.au",
        "gov.au",
        "asn.au",
        "id.au",
        "co.nz",
        "net.nz",
        "org.nz",
        "govt.nz",
        "ac.nz",
        "school.nz",
        "geek.nz",
        "co.jp",
        "ne.jp",
        "or.jp",
        "ac.jp",
        "go.jp",
        "com.cn",
        "net.cn",
        "org.cn",
        "gov.cn",
        "edu.cn",
        "com.br",
        "net.br",
        "org.br",
        "gov.br",
        "edu.br",
        "co.in",
        "net.in",
        "org.in",
        "gov.in",
        "ac.in",
    ];

    if parts.len() >= 3 {
        let potential_two_part = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
        if two_part_tlds.contains(&potential_two_part.as_str()) {
            let sld = parts[..parts.len() - 2].join(".");
            return Some((sld, potential_two_part));
        }
    }

    // Standard TLD
    let sld = parts[..parts.len() - 1].join(".");
    let tld = parts[parts.len() - 1].to_string();
    Some((sld, tld))
}

impl NamecheapProvider {
    /// Creates a new Namecheap provider with the given configuration.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use manydns::namecheap::{NamecheapProvider, ClientConfig};
    /// use manydns::types::Environment;
    ///
    /// // For sandbox testing
    /// let config = ClientConfig::sandbox("username", "api_key", "1.2.3.4");
    /// let provider = NamecheapProvider::new(config).unwrap();
    ///
    /// // For production
    /// let config = ClientConfig::production("username", "api_key", "1.2.3.4");
    /// let provider = NamecheapProvider::new(config).unwrap();
    /// ```
    pub fn new(config: ClientConfig) -> Result<Self, Box<dyn StdErr + Send + Sync>> {
        let api_client = Client::new(config)?;
        Ok(Self {
            api_client: Arc::new(api_client),
        })
    }

    /// Creates a new Namecheap provider with custom HTTP client configuration.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use manydns::namecheap::{NamecheapProvider, ClientConfig};
    /// use manydns::HttpClientConfig;
    ///
    /// let config = ClientConfig::production("username", "api_key", "1.2.3.4");
    /// let http_config = HttpClientConfig::new()
    ///     .local_address("192.168.1.100".parse().unwrap());
    /// let provider = NamecheapProvider::with_http_config(config, http_config).unwrap();
    /// ```
    pub fn with_http_config(
        config: ClientConfig,
        http_config: HttpClientConfig,
    ) -> Result<Self, Box<dyn StdErr + Send + Sync>> {
        let api_client = Client::with_http_config(config, http_config)?;
        Ok(Self {
            api_client: Arc::new(api_client),
        })
    }
}

impl Provider for NamecheapProvider {
    type Zone = NamecheapZone;
    type CustomRetrieveError = NamecheapError;

    async fn get_zone(
        &self,
        zone_id: &str,
    ) -> Result<Self::Zone, RetrieveZoneError<Self::CustomRetrieveError>> {
        let (sld, tld) = split_domain(zone_id).ok_or_else(|| {
            RetrieveZoneError::Custom(NamecheapError::Parse(format!(
                "Invalid domain format: {}",
                zone_id
            )))
        })?;

        // Verify the domain is accessible by fetching its records
        let zone = NamecheapZone {
            api_client: self.api_client.clone(),
            domain: zone_id.to_string(),
            sld,
            tld,
        };

        // Try to fetch records to verify the domain exists and is using Namecheap DNS
        zone.fetch_records().await.map_err(|e| match e {
            NamecheapError::DomainNotFound => RetrieveZoneError::NotFound,
            NamecheapError::Unauthorized => RetrieveZoneError::Unauthorized,
            other => RetrieveZoneError::Custom(other),
        })?;

        Ok(zone)
    }

    async fn list_zones(
        &self,
    ) -> Result<Vec<Self::Zone>, RetrieveZoneError<Self::CustomRetrieveError>> {
        // Namecheap doesn't have a direct "list zones" API for DNS records.
        // You would need to use namecheap.domains.getList, but that lists domains, not DNS zones.
        // For now, return an empty list as this would require additional API implementation.
        Ok(vec![])
    }
}

impl Zone for NamecheapZone {
    type CustomRetrieveError = NamecheapError;

    fn id(&self) -> &str {
        &self.domain
    }

    fn domain(&self) -> &str {
        &self.domain
    }

    async fn list_records(
        &self,
    ) -> Result<Vec<Record>, RetrieveRecordError<Self::CustomRetrieveError>> {
        let host_records = self.fetch_records().await.map_err(|e| match e {
            NamecheapError::Unauthorized => RetrieveRecordError::Unauthorized,
            other => RetrieveRecordError::Custom(other),
        })?;

        let records = host_records
            .into_iter()
            .map(|hr| host_record_to_record(hr, &self.domain))
            .collect();

        Ok(records)
    }

    async fn get_record(
        &self,
        record_id: &str,
    ) -> Result<Record, RetrieveRecordError<Self::CustomRetrieveError>> {
        let host_records = self.fetch_records().await.map_err(|e| match e {
            NamecheapError::Unauthorized => RetrieveRecordError::Unauthorized,
            other => RetrieveRecordError::Custom(other),
        })?;

        host_records
            .into_iter()
            .find(|hr| hr.host_id == record_id)
            .map(|hr| host_record_to_record(hr, &self.domain))
            .ok_or(RetrieveRecordError::NotFound)
    }
}

impl CreateRecord for NamecheapZone {
    type CustomCreateError = NamecheapError;

    async fn create_record(
        &self,
        host: &str,
        data: &RecordData,
        ttl: u64,
    ) -> Result<Record, CreateRecordError<Self::CustomCreateError>> {
        // Fetch existing records
        let mut records = self
            .fetch_records()
            .await
            .map_err(CreateRecordError::Custom)?;

        // Create new record
        let new_record = HostRecord {
            host_id: String::new(), // Will be assigned by Namecheap
            name: host.to_string(),
            record_type: data.get_type().to_string(),
            address: data.get_api_value(),
            mx_pref: if let RecordData::MX { priority, .. } = data {
                Some(*priority)
            } else {
                None
            },
            ttl: ttl.clamp(60, 60000), // Namecheap TTL range
        };

        records.push(new_record);

        // Save all records (Namecheap replaces all)
        self.save_records(&records).await.map_err(|e| match e {
            NamecheapError::Unauthorized => CreateRecordError::Unauthorized,
            other => CreateRecordError::Custom(other),
        })?;

        // Fetch updated records to get the new record with its ID
        let updated = self
            .fetch_records()
            .await
            .map_err(CreateRecordError::Custom)?;

        // Find the newly created record (last matching host/type/address)
        // Note: Namecheap may strip trailing dots from CNAME/MX values
        let expected_address = data.get_api_value().trim_end_matches('.').to_lowercase();
        let expected_type = data.get_type();

        // Debug: print what we're looking for and what we got
        #[cfg(debug_assertions)]
        {
            eprintln!(
                "DEBUG: Looking for: host='{}', type='{}', address='{}'",
                host, expected_type, expected_address
            );
            for r in &updated {
                eprintln!(
                    "DEBUG:   Found: host='{}', type='{}', address='{}'",
                    r.name,
                    r.record_type,
                    r.address.trim_end_matches('.').to_lowercase()
                );
            }
        }

        updated
            .into_iter()
            .rfind(|r| {
                r.name == host
                    && r.record_type == expected_type
                    && r.address.trim_end_matches('.').to_lowercase() == expected_address
            })
            .map(|hr| host_record_to_record(hr, &self.domain))
            .ok_or_else(|| {
                CreateRecordError::Custom(NamecheapError::Parse(
                    "Failed to find created record".to_string(),
                ))
            })
    }
}

impl DeleteRecord for NamecheapZone {
    type CustomDeleteError = NamecheapError;

    async fn delete_record(
        &self,
        record_id: &str,
    ) -> Result<(), DeleteRecordError<Self::CustomDeleteError>> {
        // Fetch existing records
        let records = self
            .fetch_records()
            .await
            .map_err(DeleteRecordError::Custom)?;

        let original_count = records.len();

        // Filter out the record to delete
        let remaining: Vec<_> = records
            .into_iter()
            .filter(|r| r.host_id != record_id)
            .collect();

        // If count is the same, record wasn't found
        if remaining.len() == original_count {
            return Err(DeleteRecordError::NotFound);
        }

        // Save remaining records
        self.save_records(&remaining).await.map_err(|e| match e {
            NamecheapError::Unauthorized => DeleteRecordError::Unauthorized,
            other => DeleteRecordError::Custom(other),
        })?;

        Ok(())
    }
}

/// Converts a Namecheap HostRecord to a manydns Record.
///
/// This is useful for custom transformations of Namecheap API responses.
pub fn host_record_to_record(hr: HostRecord, domain: &str) -> Record {
    let host = if hr.name == "@" {
        domain.to_string()
    } else {
        format!("{}.{}", hr.name, domain)
    };

    let data = match hr.record_type.as_str() {
        "MX" => RecordData::MX {
            priority: hr.mx_pref.unwrap_or(10),
            mail_server: hr.address,
        },
        _ => RecordData::from_raw(&hr.record_type, &hr.address),
    };

    Record {
        id: hr.host_id,
        host,
        data,
        ttl: hr.ttl,
    }
}
