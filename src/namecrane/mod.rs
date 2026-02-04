//! Namecrane DNS provider implementation.
//!
//! This provider uses the CraneDNS API for DNS record management.
//!
//! # Important Notes
//!
//! - **Single-Zone API**: Each API key manages exactly one domain.
//! - **No Zone Listing**: Use [`Provider::get_zone`] with the domain name directly.
//! - **Record Identification**: Records use synthesized IDs (name:type:content_hash).
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
//! let config = ClientConfig::sandbox("your-64-char-api-key", "example.org");
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
            other => RetrieveZoneError::Custom(other),
        })?;

        Ok(zone)
    }

    async fn list_zones(
        &self,
    ) -> Result<Vec<Self::Zone>, RetrieveZoneError<Self::CustomRetrieveError>> {
        // Namecrane API keys are per-domain; we can't list zones.
        // Return empty list (same pattern as Namecheap).
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
        // Parse the synthesized ID to find the record
        let (name, record_type, content_prefix) = parse_record_id(record_id).ok_or_else(|| {
            RetrieveRecordError::Custom(NamecraneError::Parse(format!(
                "Invalid record ID format: {}",
                record_id
            )))
        })?;

        // Filter by type for efficiency
        let api_records = self
            .api_client
            .list(Some(&record_type))
            .await
            .map_err(|e| match e {
                NamecraneError::Unauthorized => RetrieveRecordError::Unauthorized,
                other => RetrieveRecordError::Custom(other),
            })?;

        // Find matching record by content hash prefix
        api_records
            .into_iter()
            .find(|r| r.name == name && content_hash(&r.content).starts_with(&content_prefix))
            .map(|r| api_record_to_record(r, &self.domain))
            .ok_or(RetrieveRecordError::NotFound)
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
        let priority = match data {
            RecordData::MX { priority, .. } => Some(*priority),
            RecordData::SRV { priority, .. } => Some(*priority),
            _ => None,
        };

        self.api_client
            .create(host, record_type, &content, Some(ttl), priority)
            .await
            .map_err(|e| match e {
                NamecraneError::Unauthorized => CreateRecordError::Unauthorized,
                NamecraneError::RecordTypeNotAllowed(t) => {
                    CreateRecordError::Custom(NamecraneError::RecordTypeNotAllowed(t))
                }
                other => CreateRecordError::Custom(other),
            })?;

        // Return the created record with synthesized ID
        let full_host = if host == "@" {
            self.domain.clone()
        } else {
            format!("{}.{}", host, self.domain)
        };

        Ok(Record {
            id: synthesize_record_id(host, record_type, &content),
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
        // First, fetch the record to get its full content
        let record = self.get_record(record_id).await.map_err(|e| match e {
            RetrieveRecordError::NotFound => DeleteRecordError::NotFound,
            RetrieveRecordError::Unauthorized => DeleteRecordError::Unauthorized,
            RetrieveRecordError::Custom(c) => DeleteRecordError::Custom(c),
        })?;

        // Extract the relative host name
        let relative_host = if record.host == self.domain {
            "@".to_string()
        } else {
            record
                .host
                .strip_suffix(&format!(".{}", self.domain))
                .unwrap_or(&record.host)
                .to_string()
        };

        self.api_client
            .delete(
                &relative_host,
                record.data.get_type(),
                &record.data.get_api_value(),
            )
            .await
            .map_err(|e| match e {
                NamecraneError::Unauthorized => DeleteRecordError::Unauthorized,
                NamecraneError::RecordNotFound => DeleteRecordError::NotFound,
                other => DeleteRecordError::Custom(other),
            })?;

        Ok(())
    }
}

/// Synthesizes a record ID from its components.
///
/// Format: `name:type:content_hash` where content_hash is the first 8 chars of a hash.
fn synthesize_record_id(name: &str, record_type: &str, content: &str) -> String {
    let hash = content_hash(content);
    format!("{}:{}:{}", name, record_type, hash)
}

/// Creates a short hash of content for ID synthesis.
fn content_hash(content: &str) -> String {
    // Simple hash: sum of bytes mod 2^32, formatted as hex
    let hash: u32 = content
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_add(b as u32).wrapping_mul(31));
    format!("{:08x}", hash)
}

/// Parses a synthesized record ID back into components.
///
/// Returns (name, record_type, content_hash_prefix).
fn parse_record_id(id: &str) -> Option<(String, String, String)> {
    let parts: Vec<&str> = id.splitn(3, ':').collect();
    if parts.len() == 3 {
        Some((
            parts[0].to_string(),
            parts[1].to_string(),
            parts[2].to_string(),
        ))
    } else {
        None
    }
}

/// Converts a Namecrane API record to a manydns Record.
fn api_record_to_record(api_record: ApiRecord, domain: &str) -> Record {
    let host = if api_record.name == "@" {
        domain.to_string()
    } else {
        format!("{}.{}", api_record.name, domain)
    };

    let data = match api_record.record_type.as_str() {
        "MX" => RecordData::MX {
            priority: api_record.priority.unwrap_or(10),
            mail_server: api_record.content.clone(),
        },
        "SRV" => {
            // SRV content format: "weight port target"
            // Priority is separate field
            let parts: Vec<&str> = api_record.content.splitn(3, ' ').collect();
            if parts.len() >= 3 {
                RecordData::SRV {
                    priority: api_record.priority.unwrap_or(0),
                    weight: parts[0].parse().unwrap_or(0),
                    port: parts[1].parse().unwrap_or(0),
                    target: parts[2].to_string(),
                }
            } else {
                RecordData::from_raw(&api_record.record_type, &api_record.content)
            }
        }
        _ => RecordData::from_raw(&api_record.record_type, &api_record.content),
    };

    Record {
        id: synthesize_record_id(
            &api_record.name,
            &api_record.record_type,
            &api_record.content,
        ),
        host,
        data,
        ttl: api_record.ttl,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synthesize_and_parse_record_id() {
        let id = synthesize_record_id("www", "A", "192.168.1.1");
        let (name, rtype, hash) = parse_record_id(&id).unwrap();
        assert_eq!(name, "www");
        assert_eq!(rtype, "A");
        assert_eq!(hash.len(), 8);
    }

    #[test]
    fn test_content_hash_consistency() {
        let hash1 = content_hash("192.168.1.1");
        let hash2 = content_hash("192.168.1.1");
        assert_eq!(hash1, hash2);

        let hash3 = content_hash("192.168.1.2");
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_parse_invalid_record_id() {
        assert!(parse_record_id("invalid").is_none());
        assert!(parse_record_id("only:two").is_none());
    }

    #[test]
    fn test_api_record_to_record_apex() {
        let api = ApiRecord {
            name: "@".to_string(),
            record_type: "A".to_string(),
            content: "1.2.3.4".to_string(),
            ttl: 300,
            priority: None,
        };
        let record = api_record_to_record(api, "example.com");
        assert_eq!(record.host, "example.com");
    }

    #[test]
    fn test_api_record_to_record_subdomain() {
        let api = ApiRecord {
            name: "www".to_string(),
            record_type: "A".to_string(),
            content: "1.2.3.4".to_string(),
            ttl: 300,
            priority: None,
        };
        let record = api_record_to_record(api, "example.com");
        assert_eq!(record.host, "www.example.com");
    }
}
