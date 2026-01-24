//! Low-level Hetzner Cloud DNS API client.
//!
//! This module provides direct access to the Hetzner Cloud DNS API endpoints.
//! For most use cases, prefer using [`HetznerProvider`](super::HetznerProvider) instead.
//!
//! # API Reference
//!
//! - [Hetzner Cloud API Documentation](https://docs.hetzner.cloud/)
//! - [DNS Zones API](https://docs.hetzner.cloud/reference/cloud#zones)
//! - [DNS RRSets API](https://docs.hetzner.cloud/reference/cloud#zone-rrsets)
//!
//! # Example
//!
//! ```rust,no_run
//! use manydns::hetzner::api::Client;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = Client::new("your-api-token")?;
//!
//! // List zones
//! let response = client.retrieve_zones(1, 100).await?;
//! for zone in &response.zones {
//!     println!("Zone: {} (ID: {})", zone.name, zone.id);
//! }
//! # Ok(())
//! # }
//! ```

use std::error::Error;

use reqwest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION},
    Client as HttpClient,
};
use serde::{Deserialize, Serialize};

const HETZNER_API_URL: &str = "https://api.hetzner.cloud/v1";

/// Low-level Hetzner Cloud DNS API client.
///
/// Provides direct access to Hetzner Cloud DNS API endpoints.
/// For most use cases, prefer [`HetznerProvider`](super::HetznerProvider).
#[derive(Debug, Clone)]
pub struct Client {
    http_client: HttpClient,
    base_url: String,
}

impl Client {
    /// Creates a new Hetzner Cloud DNS API client.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Hetzner Cloud API token (Bearer token)
    pub fn new(api_key: &str) -> Result<Self, Box<dyn Error>> {
        Self::with_base_url(api_key, HETZNER_API_URL)
    }

    /// Creates a new client with a custom base URL.
    ///
    /// Primarily used for testing with mock servers.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Hetzner Cloud API token
    /// * `base_url` - Base URL for API requests
    pub fn with_base_url(api_key: &str, base_url: &str) -> Result<Self, Box<dyn Error>> {
        let mut headers = HeaderMap::new();
        let mut auth_value = HeaderValue::from_str(&format!("Bearer {}", api_key))?;
        auth_value.set_sensitive(true);
        headers.append(AUTHORIZATION, auth_value);

        let http_client = HttpClient::builder().default_headers(headers).build()?;
        Ok(Self {
            http_client,
            base_url: base_url.to_string(),
        })
    }

    /// Retrieves a paginated list of zones.
    ///
    /// # Arguments
    ///
    /// * `page` - Page number (1-indexed)
    /// * `per_page` - Number of items per page
    pub async fn retrieve_zones(
        &self,
        page: u32,
        per_page: u32,
    ) -> Result<ZonesResponse, reqwest::Error> {
        self.http_client
            .get(format!(
                "{}/zones?page={}&per_page={}",
                self.base_url, page, per_page
            ))
            .send()
            .await?
            .json::<ZonesResponse>()
            .await
    }

    /// Retrieves a zone by ID or name.
    ///
    /// # Arguments
    ///
    /// * `zone_id_or_name` - Zone identifier (numeric ID) or domain name
    pub async fn retrieve_zone(
        &self,
        zone_id_or_name: &str,
    ) -> Result<ZoneResponse, reqwest::Error> {
        self.http_client
            .get(format!("{}/zones/{}", self.base_url, zone_id_or_name))
            .send()
            .await?
            .json()
            .await
    }

    /// Creates a new zone.
    ///
    /// # Arguments
    ///
    /// * `domain` - Domain name for the zone
    /// * `ttl` - Default TTL for the zone (default: 3600)
    pub async fn create_zone(
        &self,
        domain: &str,
        ttl: Option<u64>,
    ) -> Result<CreateZoneResponse, reqwest::Error> {
        let request_body = CreateZoneRequest {
            name: domain.to_string(),
            mode: "primary".to_string(),
            ttl: ttl.unwrap_or(3600),
        };

        self.http_client
            .post(format!("{}/zones", self.base_url))
            .json(&request_body)
            .send()
            .await?
            .json()
            .await
    }

    /// Deletes a zone by ID or name.
    ///
    /// # Arguments
    ///
    /// * `zone_id_or_name` - Zone identifier or domain name
    pub async fn delete_zone(&self, zone_id_or_name: &str) -> Result<(), reqwest::Error> {
        self.http_client
            .delete(format!("{}/zones/{}", self.base_url, zone_id_or_name))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Retrieves a paginated list of RRSets in a zone.
    ///
    /// # Arguments
    ///
    /// * `zone_id_or_name` - Zone identifier or domain name
    /// * `page` - Page number (1-indexed)
    /// * `per_page` - Number of items per page (max 100)
    pub async fn retrieve_rrsets(
        &self,
        zone_id_or_name: &str,
        page: u32,
        per_page: u32,
    ) -> Result<RRSetsResponse, reqwest::Error> {
        self.http_client
            .get(format!(
                "{}/zones/{}/rrsets?page={}&per_page={}",
                self.base_url, zone_id_or_name, page, per_page
            ))
            .send()
            .await?
            .json()
            .await
    }

    /// Retrieves a specific RRSet by name and type.
    ///
    /// # Arguments
    ///
    /// * `zone_id_or_name` - Zone identifier or domain name
    /// * `rr_name` - Record name (e.g., "www" or "@" for apex)
    /// * `rr_type` - Record type (A, AAAA, CNAME, etc.)
    pub async fn retrieve_rrset(
        &self,
        zone_id_or_name: &str,
        rr_name: &str,
        rr_type: &str,
    ) -> Result<RRSetResponse, reqwest::Error> {
        self.http_client
            .get(format!(
                "{}/zones/{}/rrsets/{}/{}",
                self.base_url, zone_id_or_name, rr_name, rr_type
            ))
            .send()
            .await?
            .json()
            .await
    }

    /// Creates a new RRSet in a zone.
    ///
    /// # Arguments
    ///
    /// * `zone_id_or_name` - Zone identifier or domain name
    /// * `name` - Record name (e.g., "www" or "@" for apex)
    /// * `typ` - Record type (A, AAAA, CNAME, etc.)
    /// * `records` - List of record values
    /// * `ttl` - TTL in seconds (None uses zone default)
    pub async fn create_rrset(
        &self,
        zone_id_or_name: &str,
        name: &str,
        typ: &str,
        records: Vec<RecordValue>,
        ttl: Option<u64>,
    ) -> Result<CreateRRSetResponse, reqwest::Error> {
        let request_body = CreateRRSetRequest {
            name: name.to_string(),
            typ: typ.to_string(),
            records,
            ttl,
        };

        self.http_client
            .post(format!(
                "{}/zones/{}/rrsets",
                self.base_url, zone_id_or_name
            ))
            .json(&request_body)
            .send()
            .await?
            .json()
            .await
    }

    /// Adds records to an existing RRSet (creates it if it doesn't exist).
    ///
    /// # Arguments
    ///
    /// * `zone_id_or_name` - Zone identifier or domain name
    /// * `rr_name` - Record name (e.g., "www" or "@" for apex)
    /// * `rr_type` - Record type (A, AAAA, CNAME, etc.)
    /// * `records` - List of record values to add
    /// * `ttl` - TTL in seconds (must match existing RRSet TTL if updating)
    pub async fn add_records_to_rrset(
        &self,
        zone_id_or_name: &str,
        rr_name: &str,
        rr_type: &str,
        records: Vec<RecordValue>,
        ttl: Option<u64>,
    ) -> Result<ActionResponse, reqwest::Error> {
        let request_body = AddRecordsRequest { records, ttl };

        self.http_client
            .post(format!(
                "{}/zones/{}/rrsets/{}/{}/actions/add_records",
                self.base_url, zone_id_or_name, rr_name, rr_type
            ))
            .json(&request_body)
            .send()
            .await?
            .json()
            .await
    }

    /// Removes records from an RRSet.
    ///
    /// # Arguments
    ///
    /// * `zone_id_or_name` - Zone identifier or domain name
    /// * `rr_name` - Record name (e.g., "www" or "@" for apex)
    /// * `rr_type` - Record type (A, AAAA, CNAME, etc.)
    /// * `records` - List of record values to remove
    pub async fn remove_records_from_rrset(
        &self,
        zone_id_or_name: &str,
        rr_name: &str,
        rr_type: &str,
        records: Vec<RecordValue>,
    ) -> Result<ActionResponse, reqwest::Error> {
        let request_body = RemoveRecordsRequest { records };

        self.http_client
            .post(format!(
                "{}/zones/{}/rrsets/{}/{}/actions/remove_records",
                self.base_url, zone_id_or_name, rr_name, rr_type
            ))
            .json(&request_body)
            .send()
            .await?
            .json()
            .await
    }

    /// Deletes an entire RRSet.
    ///
    /// # Arguments
    ///
    /// * `zone_id_or_name` - Zone identifier or domain name
    /// * `rr_name` - Record name (e.g., "www" or "@" for apex)
    /// * `rr_type` - Record type (A, AAAA, CNAME, etc.)
    pub async fn delete_rrset(
        &self,
        zone_id_or_name: &str,
        rr_name: &str,
        rr_type: &str,
    ) -> Result<ActionResponse, reqwest::Error> {
        self.http_client
            .delete(format!(
                "{}/zones/{}/rrsets/{}/{}",
                self.base_url, zone_id_or_name, rr_name, rr_type
            ))
            .send()
            .await?
            .json()
            .await
    }
}

// ============================================================================
// Request Types
// ============================================================================

/// Request body for creating a zone.
#[derive(Debug, Serialize)]
struct CreateZoneRequest {
    name: String,
    mode: String,
    ttl: u64,
}

/// Request body for creating an RRSet.
#[derive(Debug, Serialize)]
struct CreateRRSetRequest {
    name: String,
    #[serde(rename = "type")]
    typ: String,
    records: Vec<RecordValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ttl: Option<u64>,
}

/// Request body for adding records to an RRSet.
#[derive(Debug, Serialize)]
struct AddRecordsRequest {
    records: Vec<RecordValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ttl: Option<u64>,
}

/// Request body for removing records from an RRSet.
#[derive(Debug, Serialize)]
struct RemoveRecordsRequest {
    records: Vec<RecordValue>,
}

// ============================================================================
// Response Types
// ============================================================================

/// A DNS zone in Hetzner Cloud.
#[derive(Debug, PartialEq, Eq, Clone, Hash, Deserialize)]
pub struct Zone {
    /// Unique zone identifier (numeric).
    pub id: u64,
    /// Domain name (e.g., "example.com").
    pub name: String,
    /// Zone mode (primary or secondary).
    pub mode: String,
    /// Zone status.
    pub status: ZoneStatus,
    /// Default TTL for records in this zone.
    pub ttl: u64,
    /// Number of records in the zone.
    #[serde(default)]
    pub record_count: u32,
}

/// Zone status in Hetzner Cloud DNS.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ZoneStatus {
    /// Zone is active and working.
    Ok,
    /// Zone is pending setup.
    Pending,
    /// Zone verification failed.
    Failed,
}

/// Response wrapper for a single zone.
#[derive(Debug, PartialEq, Eq, Clone, Hash, Deserialize)]
pub struct ZoneResponse {
    /// The zone data.
    pub zone: Zone,
}

/// Response wrapper for creating a zone.
#[derive(Debug, PartialEq, Eq, Clone, Hash, Deserialize)]
pub struct CreateZoneResponse {
    /// The created zone.
    pub zone: Zone,
    /// The action tracking the creation.
    pub action: Action,
}

/// Response wrapper for listing zones.
#[derive(Debug, PartialEq, Eq, Clone, Hash, Deserialize)]
pub struct ZonesResponse {
    /// Pagination metadata.
    pub meta: Meta,
    /// List of zones.
    pub zones: Vec<Zone>,
}

/// A DNS RRSet (Resource Record Set) in Hetzner Cloud.
///
/// An RRSet is a collection of DNS records with the same name and type.
#[derive(Debug, PartialEq, Eq, Clone, Hash, Deserialize)]
pub struct RRSet {
    /// Unique RRSet identifier (format: "name/type").
    pub id: String,
    /// Record name (e.g., "www" or "@" for apex).
    pub name: String,
    /// Record type (A, AAAA, CNAME, etc.).
    #[serde(rename = "type")]
    pub typ: String,
    /// TTL in seconds (None uses zone default).
    pub ttl: Option<u64>,
    /// List of record values.
    pub records: Vec<RecordValue>,
    /// Zone ID this RRSet belongs to.
    pub zone: u64,
}

/// A single record value within an RRSet.
#[derive(Debug, PartialEq, Eq, Clone, Hash, Serialize, Deserialize)]
pub struct RecordValue {
    /// The record value (e.g., IP address for A records).
    pub value: String,
    /// Optional comment for this record.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

impl RecordValue {
    /// Creates a new record value without a comment.
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            comment: None,
        }
    }

    /// Creates a new record value with a comment.
    pub fn with_comment(value: impl Into<String>, comment: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            comment: Some(comment.into()),
        }
    }
}

/// Response wrapper for a single RRSet.
#[derive(Debug, PartialEq, Eq, Clone, Hash, Deserialize)]
pub struct RRSetResponse {
    /// The RRSet data.
    pub rrset: RRSet,
}

/// Response wrapper for creating an RRSet.
#[derive(Debug, PartialEq, Eq, Clone, Hash, Deserialize)]
pub struct CreateRRSetResponse {
    /// The created RRSet.
    pub rrset: RRSet,
    /// The action tracking the creation.
    pub action: Action,
}

/// Response wrapper for listing RRSets.
#[derive(Debug, PartialEq, Eq, Clone, Hash, Deserialize)]
pub struct RRSetsResponse {
    /// Pagination metadata.
    pub meta: Meta,
    /// List of RRSets.
    pub rrsets: Vec<RRSet>,
}

/// Response wrapper for actions.
#[derive(Debug, PartialEq, Eq, Clone, Hash, Deserialize)]
pub struct ActionResponse {
    /// The action data.
    pub action: Action,
}

/// An async action in Hetzner Cloud.
#[derive(Debug, PartialEq, Eq, Clone, Hash, Deserialize)]
pub struct Action {
    /// Unique action identifier.
    pub id: u64,
    /// Command that was executed.
    pub command: String,
    /// Current status.
    pub status: ActionStatus,
    /// Progress percentage (0-100).
    pub progress: u32,
}

/// Status of an action in Hetzner Cloud.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActionStatus {
    /// Action is currently running.
    Running,
    /// Action completed successfully.
    Success,
    /// Action failed.
    Error,
}

/// Pagination metadata for list responses.
#[derive(Debug, PartialEq, Eq, Clone, Hash, Deserialize)]
pub struct Meta {
    /// Pagination details.
    pub pagination: Pagination,
}

/// Pagination details for list responses.
#[derive(Debug, PartialEq, Eq, Clone, Hash, Deserialize)]
pub struct Pagination {
    /// Last page number (1-indexed).
    pub last_page: u32,
    /// Current page number (1-indexed).
    pub page: u32,
    /// Items per page.
    pub per_page: u32,
    /// Total number of entries across all pages.
    pub total_entries: u32,
}
