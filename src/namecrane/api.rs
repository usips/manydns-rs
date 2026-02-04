//! Namecrane API client implementation.
//!
//! This module provides a low-level HTTP client for the Namecrane (CraneDNS) API.

use std::error::Error;
use std::fmt;
use std::time::Duration;

use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};

use crate::types::Environment;
use crate::HttpClientConfig;

/// Namecrane API endpoints.
const PRODUCTION_API_URL: &str = "https://namecrane.com/index.php?m=cranedns&action=api";
const SANDBOX_API_URL: &str = "https://namecrane.org/index.php?m=cranedns&action=api";

/// Errors that can occur when using the Namecrane API.
#[derive(Debug)]
pub enum NamecraneError {
    /// HTTP request error.
    Request(reqwest::Error),
    /// API error response.
    Api(String),
    /// JSON parsing error.
    Parse(String),
    /// Unauthorized access (invalid API key).
    Unauthorized,
    /// Record not found.
    RecordNotFound,
    /// Record type not allowed.
    RecordTypeNotAllowed(String),
}

impl fmt::Display for NamecraneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NamecraneError::Request(e) => write!(f, "HTTP request error: {}", e),
            NamecraneError::Api(msg) => write!(f, "API error: {}", msg),
            NamecraneError::Parse(msg) => write!(f, "Parse error: {}", msg),
            NamecraneError::Unauthorized => write!(f, "Unauthorized (invalid API key)"),
            NamecraneError::RecordNotFound => write!(f, "Record not found"),
            NamecraneError::RecordTypeNotAllowed(t) => write!(f, "Record type not allowed: {}", t),
        }
    }
}

impl Error for NamecraneError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            NamecraneError::Request(e) => Some(e),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for NamecraneError {
    fn from(err: reqwest::Error) -> Self {
        NamecraneError::Request(err)
    }
}

/// A DNS record from the Namecrane API.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiRecord {
    /// The hostname/subdomain (e.g., "@", "www", "mail").
    pub name: String,
    /// Record type (A, AAAA, CNAME, MX, TXT, etc.).
    #[serde(rename = "type")]
    pub record_type: String,
    /// The record value (IP address, hostname, etc.).
    pub content: String,
    /// TTL in seconds.
    pub ttl: u64,
    /// Priority (for MX/SRV records).
    #[serde(default)]
    pub priority: Option<u16>,
}

/// API response wrapper.
#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    success: bool,
    #[serde(flatten)]
    data: Option<T>,
    error: Option<String>,
    message: Option<String>,
}

/// List records response data.
#[derive(Debug, Deserialize)]
struct ListData {
    records: Vec<ApiRecord>,
    #[allow(dead_code)]
    domain: Option<String>,
}

/// Request body for creating a record.
#[derive(Debug, Serialize)]
struct CreateRequest {
    name: String,
    #[serde(rename = "type")]
    record_type: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    ttl: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    priority: Option<u16>,
}

/// Request body for deleting a record.
#[derive(Debug, Serialize)]
struct DeleteRequest {
    name: String,
    #[serde(rename = "type")]
    record_type: String,
    content: String,
}

/// Configuration for the Namecrane API client.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// API key (64 characters).
    pub api_key: String,
    /// The domain this API key manages.
    pub domain: String,
    /// API environment (sandbox or production).
    pub environment: Environment,
}

impl ClientConfig {
    /// Creates a new client configuration.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Your Namecrane API key (64 characters)
    /// * `domain` - The domain this API key manages
    /// * `environment` - Sandbox or Production
    pub fn new(
        api_key: impl Into<String>,
        domain: impl Into<String>,
        environment: Environment,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            domain: domain.into(),
            environment,
        }
    }

    /// Creates a configuration for the sandbox environment.
    pub fn sandbox(api_key: impl Into<String>, domain: impl Into<String>) -> Self {
        Self::new(api_key, domain, Environment::Sandbox)
    }

    /// Creates a configuration for the production environment.
    pub fn production(api_key: impl Into<String>, domain: impl Into<String>) -> Self {
        Self::new(api_key, domain, Environment::Production)
    }

    /// Returns the API base URL for the configured environment.
    pub fn api_url(&self) -> &'static str {
        match self.environment {
            Environment::Production => PRODUCTION_API_URL,
            Environment::Sandbox => SANDBOX_API_URL,
        }
    }
}

/// Namecrane API client.
#[derive(Debug, Clone)]
pub struct Client {
    http_client: HttpClient,
    api_key: String,
    base_url: &'static str,
    domain: String,
}

impl Client {
    /// Creates a new Namecrane API client.
    pub fn new(config: ClientConfig) -> Result<Self, Box<dyn Error + Send + Sync>> {
        Self::with_http_config(config, HttpClientConfig::default())
    }

    /// Creates a new Namecrane API client with custom HTTP configuration.
    pub fn with_http_config(
        config: ClientConfig,
        http_config: HttpClientConfig,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let mut builder = HttpClient::builder()
            .user_agent("manydns-rs/1.1.1")
            .timeout(http_config.timeout.unwrap_or(Duration::from_secs(30)));

        if let Some(addr) = http_config.local_address {
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
        if let Some(ref iface) = http_config.interface {
            builder = builder.interface(iface);
        }

        let http_client = builder.build()?;
        let base_url = config.api_url();

        Ok(Self {
            http_client,
            api_key: config.api_key,
            base_url,
            domain: config.domain,
        })
    }

    /// Returns the domain this client manages.
    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Lists all DNS records, optionally filtered by type.
    pub async fn list(&self, record_type: Option<&str>) -> Result<Vec<ApiRecord>, NamecraneError> {
        let mut url = format!("{}&method=list", self.base_url);
        if let Some(rt) = record_type {
            url.push_str("&type=");
            url.push_str(rt);
        }

        let response = self
            .http_client
            .get(&url)
            .header("X-API-Key", &self.api_key)
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await?;

        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(NamecraneError::Unauthorized);
        }

        let api_response: ApiResponse<ListData> = serde_json::from_str(&text).map_err(|e| {
            NamecraneError::Parse(format!("Failed to parse response: {} - {}", e, text))
        })?;

        if !api_response.success {
            let error_msg = api_response
                .error
                .or(api_response.message)
                .unwrap_or_else(|| "Unknown error".to_string());
            return Err(NamecraneError::Api(error_msg));
        }

        Ok(api_response.data.map(|d| d.records).unwrap_or_default())
    }

    /// Creates a new DNS record.
    pub async fn create(
        &self,
        name: &str,
        record_type: &str,
        content: &str,
        ttl: Option<u64>,
        priority: Option<u16>,
    ) -> Result<(), NamecraneError> {
        let url = format!("{}&method=create", self.base_url);

        let request = CreateRequest {
            name: name.to_string(),
            record_type: record_type.to_string(),
            content: content.to_string(),
            ttl,
            priority,
        };

        let response = self
            .http_client
            .post(&url)
            .header("X-API-Key", &self.api_key)
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await?;

        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            // Check if it's a record type restriction
            if text.contains("not allowed") || text.contains("record type") {
                return Err(NamecraneError::RecordTypeNotAllowed(
                    record_type.to_string(),
                ));
            }
            return Err(NamecraneError::Unauthorized);
        }

        let api_response: ApiResponse<serde_json::Value> =
            serde_json::from_str(&text).map_err(|e| {
                NamecraneError::Parse(format!("Failed to parse response: {} - {}", e, text))
            })?;

        if !api_response.success {
            let error_msg = api_response
                .error
                .or(api_response.message)
                .unwrap_or_else(|| "Unknown error".to_string());
            return Err(NamecraneError::Api(error_msg));
        }

        Ok(())
    }

    /// Deletes a DNS record by its composite key.
    pub async fn delete(
        &self,
        name: &str,
        record_type: &str,
        content: &str,
    ) -> Result<(), NamecraneError> {
        let url = format!("{}&method=delete", self.base_url);

        let request = DeleteRequest {
            name: name.to_string(),
            record_type: record_type.to_string(),
            content: content.to_string(),
        };

        let response = self
            .http_client
            .post(&url)
            .header("X-API-Key", &self.api_key)
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await?;

        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(NamecraneError::Unauthorized);
        }

        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(NamecraneError::RecordNotFound);
        }

        let api_response: ApiResponse<serde_json::Value> =
            serde_json::from_str(&text).map_err(|e| {
                NamecraneError::Parse(format!("Failed to parse response: {} - {}", e, text))
            })?;

        if !api_response.success {
            let error_msg = api_response
                .error
                .or(api_response.message)
                .unwrap_or_else(|| "Unknown error".to_string());

            // Check for "not found" style messages
            if error_msg.to_lowercase().contains("not found") {
                return Err(NamecraneError::RecordNotFound);
            }
            return Err(NamecraneError::Api(error_msg));
        }

        Ok(())
    }
}
