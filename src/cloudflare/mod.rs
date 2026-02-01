//! Cloudflare DNS provider implementation.
//!
//! This provider uses the Cloudflare API with Bearer token authentication.
//!
//! # Authentication
//!
//! Requires a Cloudflare API token:
//! - Create a token with DNS read/write permissions at: <https://dash.cloudflare.com/profile/api-tokens>
//!
//! # Example
//!
//! ```no_run
//! use manydns::cloudflare::CloudflareProvider;
//! use manydns::{Provider, Zone};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! let provider = CloudflareProvider::new("your_api_token")?;
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
//! - [DNS Records API](https://developers.cloudflare.com/api/resources/dns/subresources/records/)
//! - [Zones API](https://developers.cloudflare.com/api/resources/zones/)

pub mod api;

use std::error::Error as StdErr;
use std::sync::Arc;

pub use api::{ApiError, Client, CloudflareError, DnsRecordWithZone, RecordConversionError};

use crate::{
    CreateRecord, CreateRecordError, DeleteRecord, DeleteRecordError, HttpClientConfig, Provider,
    Record, RecordData, RetrieveRecordError, RetrieveZoneError, Zone,
};

/// Cloudflare DNS provider.
///
/// Uses the Cloudflare API with Bearer token authentication.
#[derive(Clone)]
pub struct CloudflareProvider {
    api_client: Arc<Client>,
}

/// A DNS zone managed by Cloudflare.
pub struct CloudflareZone {
    api_client: Arc<Client>,
    /// The zone info.
    repr: api::Zone,
}

impl CloudflareZone {
    /// Returns the domain name.
    pub fn domain(&self) -> &str {
        &self.repr.name
    }
}

impl CloudflareProvider {
    /// Creates a new Cloudflare provider.
    ///
    /// # Arguments
    ///
    /// * `api_token` - Cloudflare API token (Bearer token)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use manydns::cloudflare::CloudflareProvider;
    ///
    /// let provider = CloudflareProvider::new("your_api_token").unwrap();
    /// ```
    pub fn new(api_token: &str) -> Result<Self, Box<dyn StdErr + Send + Sync>> {
        let api_client = Client::new(api_token)?;
        Ok(Self {
            api_client: Arc::new(api_client),
        })
    }

    /// Creates a new Cloudflare provider with custom HTTP client configuration.
    ///
    /// This allows binding to a specific local IP address or network interface.
    ///
    /// # Arguments
    ///
    /// * `api_token` - Cloudflare API token (Bearer token)
    /// * `config` - HTTP client configuration
    ///
    /// # Example
    ///
    /// ```no_run
    /// use manydns::cloudflare::CloudflareProvider;
    /// use manydns::HttpClientConfig;
    ///
    /// // Bind to a specific source IP
    /// let config = HttpClientConfig::new()
    ///     .local_address("192.168.1.100".parse().unwrap());
    /// let provider = CloudflareProvider::with_config("your_api_token", config).unwrap();
    /// ```
    pub fn with_config(
        api_token: &str,
        config: HttpClientConfig,
    ) -> Result<Self, Box<dyn StdErr + Send + Sync>> {
        let api_client = Client::with_config(api_token, config)?;
        Ok(Self {
            api_client: Arc::new(api_client),
        })
    }

    /// Creates a new Cloudflare provider with a custom API base URL.
    ///
    /// This is primarily useful for testing with mock servers.
    ///
    /// # Arguments
    ///
    /// * `api_token` - Cloudflare API token (Bearer token)
    /// * `base_url` - Custom base URL for the API
    pub fn with_base_url(
        api_token: &str,
        base_url: &str,
    ) -> Result<Self, Box<dyn StdErr + Send + Sync>> {
        let api_client = Client::with_base_url(api_token, base_url, HttpClientConfig::default())?;
        Ok(Self {
            api_client: Arc::new(api_client),
        })
    }
}

impl Provider for CloudflareProvider {
    type Zone = CloudflareZone;
    type CustomRetrieveError = CloudflareError;

    async fn get_zone(
        &self,
        zone_id: &str,
    ) -> Result<Self::Zone, RetrieveZoneError<Self::CustomRetrieveError>> {
        // zone_id can be either a zone ID (32-char hex) or domain name
        let zone = if zone_id.len() == 32 && zone_id.chars().all(|c| c.is_ascii_hexdigit()) {
            self.api_client.get_zone(zone_id).await
        } else {
            self.api_client.get_zone_by_name(zone_id).await
        };

        let zone = zone.map_err(|err| match &err {
            CloudflareError::Api(api_err) => match api_err.code {
                // 9109 = Zone not found, 7003 = Could not find zone
                9109 | 7003 | 1003 => RetrieveZoneError::NotFound,
                // 9106 = Missing X-Auth headers, 10000 = Authentication error
                9106 | 10000 => RetrieveZoneError::Unauthorized,
                _ => RetrieveZoneError::Custom(err),
            },
            _ => RetrieveZoneError::Custom(err),
        })?;

        Ok(CloudflareZone {
            api_client: self.api_client.clone(),
            repr: zone,
        })
    }

    async fn list_zones(
        &self,
    ) -> Result<Vec<Self::Zone>, RetrieveZoneError<Self::CustomRetrieveError>> {
        let zones = self
            .api_client
            .list_zones()
            .await
            .map_err(|err| match &err {
                CloudflareError::Api(api_err) => match api_err.code {
                    9106 | 10000 => RetrieveZoneError::Unauthorized,
                    _ => RetrieveZoneError::Custom(err),
                },
                _ => RetrieveZoneError::Custom(err),
            })?;

        Ok(zones
            .into_iter()
            .map(|zone| CloudflareZone {
                api_client: self.api_client.clone(),
                repr: zone,
            })
            .collect())
    }
}

impl Zone for CloudflareZone {
    type CustomRetrieveError = CloudflareError;

    fn id(&self) -> &str {
        &self.repr.id
    }

    fn domain(&self) -> &str {
        &self.repr.name
    }

    async fn list_records(
        &self,
    ) -> Result<Vec<Record>, RetrieveRecordError<Self::CustomRetrieveError>> {
        let records =
            self.api_client
                .list_records(&self.repr.id)
                .await
                .map_err(|err| match &err {
                    CloudflareError::Api(api_err) => match api_err.code {
                        9106 | 10000 => RetrieveRecordError::Unauthorized,
                        _ => RetrieveRecordError::Custom(err),
                    },
                    _ => RetrieveRecordError::Custom(err),
                })?;

        Ok(records
            .into_iter()
            .filter_map(|r| {
                crate::Record::try_from(api::DnsRecordWithZone::new(&r, &self.repr.name)).ok()
            })
            .collect())
    }

    async fn get_record(
        &self,
        record_id: &str,
    ) -> Result<Record, RetrieveRecordError<Self::CustomRetrieveError>> {
        let record = self
            .api_client
            .get_record(&self.repr.id, record_id)
            .await
            .map_err(|err| match &err {
                CloudflareError::Api(api_err) => match api_err.code {
                    // 81044 = Record not found
                    81044 => RetrieveRecordError::NotFound,
                    9106 | 10000 => RetrieveRecordError::Unauthorized,
                    _ => RetrieveRecordError::Custom(err),
                },
                _ => RetrieveRecordError::Custom(err),
            })?;

        crate::Record::try_from(api::DnsRecordWithZone::new(&record, &self.repr.name)).map_err(
            |e| {
                RetrieveRecordError::Custom(CloudflareError::Api(ApiError {
                    code: 0,
                    message: format!("Failed to convert record: {}", e),
                }))
            },
        )
    }
}

impl CreateRecord for CloudflareZone {
    type CustomCreateError = CloudflareError;

    async fn create_record(
        &self,
        host: &str,
        data: &RecordData,
        ttl: u64,
    ) -> Result<Record, CreateRecordError<Self::CustomCreateError>> {
        let request = api::CreateRecordRequest::from_record_data(host, data, ttl, &self.repr.name)
            .map_err(|_| CreateRecordError::UnsupportedType)?;

        let record = self
            .api_client
            .create_record(&self.repr.id, &request)
            .await
            .map_err(|err| match &err {
                CloudflareError::Api(api_err) => match api_err.code {
                    9106 | 10000 => CreateRecordError::Unauthorized,
                    // 81057 = Record already exists
                    81057 => CreateRecordError::InvalidRecord,
                    _ => CreateRecordError::Custom(err),
                },
                _ => CreateRecordError::Custom(err),
            })?;

        crate::Record::try_from(api::DnsRecordWithZone::new(&record, &self.repr.name)).map_err(
            |e| {
                CreateRecordError::Custom(CloudflareError::Api(ApiError {
                    code: 0,
                    message: format!("Failed to convert record: {}", e),
                }))
            },
        )
    }
}

impl DeleteRecord for CloudflareZone {
    type CustomDeleteError = CloudflareError;

    async fn delete_record(
        &self,
        record_id: &str,
    ) -> Result<(), DeleteRecordError<Self::CustomDeleteError>> {
        self.api_client
            .delete_record(&self.repr.id, record_id)
            .await
            .map_err(|err| match &err {
                CloudflareError::Api(api_err) => match api_err.code {
                    // 81044 = Record not found
                    81044 => DeleteRecordError::NotFound,
                    9106 | 10000 => DeleteRecordError::Unauthorized,
                    _ => DeleteRecordError::Custom(err),
                },
                _ => DeleteRecordError::Custom(err),
            })?;

        Ok(())
    }
}
