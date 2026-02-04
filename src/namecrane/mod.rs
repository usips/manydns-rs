//! Namecrane DNS provider implementation.
//!
//! This provider uses the CraneDNS API for DNS record management.
//!
//! # Important Notes
//!
//! - **Domain-Based Access**: Specify the domain when creating the provider.
//! - **No Zone Listing**: Use [`Provider::get_zone`] with the domain name directly.
//! - **Record IDs**: Records have stable UUIDs assigned by the API.
//!
//! # Environments
//!
//! - Production: `namecrane.com`
//! - Sandbox: `namecrane.org`
//!
//! # Example
//!
//! ```no_run
//! use manydns::namecrane::{NamecraneProvider, ClientConfig};
//! use manydns::{Provider, Zone};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! let config = ClientConfig::sandbox("your-api-key", "example.org");
//! let provider = NamecraneProvider::new(config)?;
//!
//! let zone = provider.get_zone("example.org").await?;
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
//! A, AAAA, CNAME, MX, TXT, SRV, CAA, NS, DS, DNSKEY
//!
//! # API Reference
//!
//! - [GitHub: namecrane/dns-api](https://github.com/namecrane/dns-api)

pub mod api;

use std::error::Error as StdErr;
use std::sync::Arc;

pub use api::{ApiRecord, Client, ClientConfig, NamecraneError};

use crate::{
    CreateRecord, CreateRecordError, DeleteRecord, DeleteRecordError, HttpClientConfig, Provider,
    Record, RecordData, RetrieveRecordError, RetrieveZoneError, Zone,
};

/// Namecrane DNS provider.
///
/// Manages DNS records through the CraneDNS API.
#[derive(Clone)]
pub struct NamecraneProvider {
    api_client: Arc<Client>,
    domain: String,
}

/// A DNS zone managed by Namecrane.
///
/// The zone represents a domain (e.g., "example.com") and provides
/// methods to manage its DNS records.
pub struct NamecraneZone {
    api_client: Arc<Client>,
    /// The domain name (e.g., "example.com").
    domain: String,
}

impl NamecraneZone {
    /// Returns the domain name.
    pub fn domain(&self) -> &str {
        &self.domain
    }
}

impl NamecraneProvider {
    /// Creates a new Namecrane provider with the given configuration.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use manydns::namecrane::{NamecraneProvider, ClientConfig};
    ///
    /// // For sandbox testing
    /// let config = ClientConfig::sandbox("your-api-key", "example.org");
    /// let provider = NamecraneProvider::new(config).unwrap();
    ///
    /// // For production
    /// let config = ClientConfig::production("your-api-key", "example.com");
    /// let provider = NamecraneProvider::new(config).unwrap();
    /// ```
    pub fn new(config: ClientConfig) -> Result<Self, Box<dyn StdErr + Send + Sync>> {
        let domain = config.domain.clone();
        let api_client = Client::new(config)?;
        Ok(Self {
            api_client: Arc::new(api_client),
            domain,
        })
    }

    /// Creates a new Namecrane provider with custom HTTP client configuration.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use manydns::namecrane::{NamecraneProvider, ClientConfig};
    /// use manydns::HttpClientConfig;
    ///
    /// let config = ClientConfig::production("your-api-key", "example.com");
    /// let http_config = HttpClientConfig::new()
    ///     .local_address("192.168.1.100".parse().unwrap());
    /// let provider = NamecraneProvider::with_http_config(config, http_config).unwrap();
    /// ```
    pub fn with_http_config(
        config: ClientConfig,
        http_config: HttpClientConfig,
    ) -> Result<Self, Box<dyn StdErr + Send + Sync>> {
        let domain = config.domain.clone();
        let api_client = Client::with_http_config(config, http_config)?;
        Ok(Self {
            api_client: Arc::new(api_client),
            domain,
        })
    }
}

impl Provider for NamecraneProvider {
    type Zone = NamecraneZone;
    type CustomRetrieveError = NamecraneError;

    async fn get_zone(
        &self,
        zone_id: &str,
    ) -> Result<Self::Zone, RetrieveZoneError<Self::CustomRetrieveError>> {
        // Verify the requested zone matches the configured domain
        if zone_id != self.domain {
            return Err(RetrieveZoneError::NotFound);
        }

        let zone = NamecraneZone {
            api_client: self.api_client.clone(),
            domain: self.domain.clone(),
        };

        // Verify access by attempting to list records
        zone.api_client.list(None).await.map_err(|e| match e {
            NamecraneError::Unauthorized => RetrieveZoneError::Unauthorized,
            NamecraneError::DomainNotFound => RetrieveZoneError::NotFound,
            NamecraneError::Forbidden(_) => RetrieveZoneError::Unauthorized,
            other => RetrieveZoneError::Custom(other),
        })?;

        Ok(zone)
    }

    async fn list_zones(
        &self,
    ) -> Result<Vec<Self::Zone>, RetrieveZoneError<Self::CustomRetrieveError>> {
        // Namecrane API doesn't support listing zones; return empty.
        // Use get_zone() with the domain name directly.
        Ok(vec![])
    }
}

impl Zone for NamecraneZone {
    type CustomRetrieveError = NamecraneError;

    fn id(&self) -> &str {
        &self.domain
    }

    fn domain(&self) -> &str {
        &self.domain
    }

    async fn list_records(
        &self,
    ) -> Result<Vec<Record>, RetrieveRecordError<Self::CustomRetrieveError>> {
        let api_records = self.api_client.list(None).await.map_err(|e| match e {
            NamecraneError::Unauthorized => RetrieveRecordError::Unauthorized,
            NamecraneError::Forbidden(_) => RetrieveRecordError::Unauthorized,
            other => RetrieveRecordError::Custom(other),
        })?;

        let records = api_records
            .into_iter()
            .map(|r| api_record_to_record(r, &self.domain))
            .collect();

        Ok(records)
    }

    async fn get_record(
        &self,
        record_id: &str,
    ) -> Result<Record, RetrieveRecordError<Self::CustomRetrieveError>> {
        let api_record = self.api_client.get(record_id).await.map_err(|e| match e {
            NamecraneError::Unauthorized => RetrieveRecordError::Unauthorized,
            NamecraneError::Forbidden(_) => RetrieveRecordError::Unauthorized,
            NamecraneError::RecordNotFound => RetrieveRecordError::NotFound,
            other => RetrieveRecordError::Custom(other),
        })?;

        Ok(api_record_to_record(api_record, &self.domain))
    }
}

impl CreateRecord for NamecraneZone {
    type CustomCreateError = NamecraneError;

    async fn create_record(
        &self,
        host: &str,
        data: &RecordData,
        ttl: u64,
    ) -> Result<Record, CreateRecordError<Self::CustomCreateError>> {
        let record_type = data.get_type();
        let content = data.get_api_value();

        let record_id = self
            .api_client
            .create(host, record_type, &content, Some(ttl))
            .await
            .map_err(|e| match e {
                NamecraneError::Unauthorized => CreateRecordError::Unauthorized,
                NamecraneError::Forbidden(_) => CreateRecordError::Unauthorized,
                other => CreateRecordError::Custom(other),
            })?;

        // Build the full host name
        let full_host = if host == "@" {
            self.domain.clone()
        } else {
            format!("{}.{}", host, self.domain)
        };

        Ok(Record {
            id: record_id,
            host: full_host,
            data: data.clone(),
            ttl,
        })
    }
}

impl DeleteRecord for NamecraneZone {
    type CustomDeleteError = NamecraneError;

    async fn delete_record(
        &self,
        record_id: &str,
    ) -> Result<(), DeleteRecordError<Self::CustomDeleteError>> {
        self.api_client.delete(record_id).await.map_err(|e| match e {
            NamecraneError::Unauthorized => DeleteRecordError::Unauthorized,
            NamecraneError::Forbidden(_) => DeleteRecordError::Unauthorized,
            NamecraneError::RecordNotFound => DeleteRecordError::NotFound,
            other => DeleteRecordError::Custom(other),
        })?;

        Ok(())
    }
}

/// Converts a Namecrane API record to a manydns Record.
fn api_record_to_record(api_record: ApiRecord, domain: &str) -> Record {
    let host = if api_record.name == "@" {
        domain.to_string()
    } else {
        format!("{}.{}", api_record.name, domain)
    };

    // Parse MX records specially (content format: "priority server")
    let data = match api_record.record_type.as_str() {
        "MX" => {
            let parts: Vec<&str> = api_record.content.splitn(2, ' ').collect();
            if parts.len() == 2 {
                if let Ok(priority) = parts[0].parse::<u16>() {
                    RecordData::MX {
                        priority,
                        mail_server: parts[1].to_string(),
                    }
                } else {
                    RecordData::from_raw(&api_record.record_type, &api_record.content)
                }
            } else {
                RecordData::from_raw(&api_record.record_type, &api_record.content)
            }
        }
        "SRV" => {
            // SRV content format: "priority weight port target"
            let parts: Vec<&str> = api_record.content.splitn(4, ' ').collect();
            if parts.len() == 4 {
                if let (Ok(priority), Ok(weight), Ok(port)) = (
                    parts[0].parse::<u16>(),
                    parts[1].parse::<u16>(),
                    parts[2].parse::<u16>(),
                ) {
                    RecordData::SRV {
                        priority,
                        weight,
                        port,
                        target: parts[3].to_string(),
                    }
                } else {
                    RecordData::from_raw(&api_record.record_type, &api_record.content)
                }
            } else {
                RecordData::from_raw(&api_record.record_type, &api_record.content)
            }
        }
        _ => RecordData::from_raw(&api_record.record_type, &api_record.content),
    };

    Record {
        id: api_record.id,
        host,
        data,
        ttl: api_record.ttl,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_record_to_record_apex() {
        let api = ApiRecord {
            id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            name: "@".to_string(),
            record_type: "A".to_string(),
            content: "1.2.3.4".to_string(),
            ttl: 300,
        };
        let record = api_record_to_record(api, "example.com");
        assert_eq!(record.host, "example.com");
        assert_eq!(record.id, "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn test_api_record_to_record_subdomain() {
        let api = ApiRecord {
            id: "6ba7b810-9dad-11d1-80b4-00c04fd430c8".to_string(),
            name: "www".to_string(),
            record_type: "A".to_string(),
            content: "1.2.3.4".to_string(),
            ttl: 300,
        };
        let record = api_record_to_record(api, "example.com");
        assert_eq!(record.host, "www.example.com");
    }

    #[test]
    fn test_api_record_to_record_mx() {
        let api = ApiRecord {
            id: "f47ac10b-58cc-4372-a567-0e02b2c3d479".to_string(),
            name: "@".to_string(),
            record_type: "MX".to_string(),
            content: "10 mail.example.com.".to_string(),
            ttl: 3600,
        };
        let record = api_record_to_record(api, "example.com");
        assert!(matches!(record.data, RecordData::MX { priority: 10, .. }));
    }

    #[test]
    fn test_api_record_to_record_srv() {
        let api = ApiRecord {
            id: "a1b2c3d4-5678-90ab-cdef-1234567890ab".to_string(),
            name: "_sip._tcp".to_string(),
            record_type: "SRV".to_string(),
            content: "10 5 5060 sipserver.example.com.".to_string(),
            ttl: 3600,
        };
        let record = api_record_to_record(api, "example.com");
        assert!(matches!(
            record.data,
            RecordData::SRV {
                priority: 10,
                weight: 5,
                port: 5060,
                ..
            }
        ));
    }
}
