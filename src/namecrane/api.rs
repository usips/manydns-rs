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
const PRODUCTION_API_URL: &str = "https://namecrane.com/index.php?m=craneapi";
const SANDBOX_API_URL: &str = "https://namecrane.org/index.php?m=craneapi";

/// Errors that can occur when using the Namecrane API.
#[derive(Debug)]
pub enum NamecraneError {
    /// HTTP request error.
    Request(reqwest::Error),
    /// API error response.
    Api { message: String, code: u16 },
    /// JSON parsing error.
    Parse(String),
    /// Unauthorized access (invalid API key).
    Unauthorized,
    /// Domain not found or not authorized.
    DomainNotFound,
    /// Record not found.
    RecordNotFound,
    /// Forbidden (IP not whitelisted, insufficient access).
    Forbidden(String),
}

impl fmt::Display for NamecraneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NamecraneError::Request(e) => write!(f, "HTTP request error: {}", e),
            NamecraneError::Api { message, code } => write!(f, "API error ({}): {}", code, message),
            NamecraneError::Parse(msg) => write!(f, "Parse error: {}", msg),
            NamecraneError::Unauthorized => write!(f, "Unauthorized (invalid API key)"),
            NamecraneError::DomainNotFound => write!(f, "Domain not found or not authorized"),
            NamecraneError::RecordNotFound => write!(f, "Record not found"),
            NamecraneError::Forbidden(msg) => write!(f, "Forbidden: {}", msg),
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
    /// Unique record ID (UUID).
    pub id: String,
    /// The hostname/subdomain (e.g., "@", "www", "mail").
    pub name: String,
    /// Record type (A, AAAA, CNAME, MX, TXT, etc.).
    #[serde(rename = "type")]
    pub record_type: String,
    /// The record value (IP address, hostname, etc.).
    pub content: String,
    /// TTL in seconds.
    pub ttl: u64,
}

/// API response wrapper.
#[derive(Debug, Deserialize)]
struct ApiResponse {
    success: bool,
    #[serde(default)]
    records: Option<Vec<ApiRecord>>,
    #[serde(default)]
    record: Option<ApiRecord>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    code: Option<u16>,
}

/// Request body for dns.list action.
#[derive(Debug, Serialize)]
struct ListRequest<'a> {
    action: &'static str,
    domain: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    record_type: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<&'a str>,
}

/// Request body for dns.create action.
#[derive(Debug, Serialize)]
struct CreateRequest<'a> {
    action: &'static str,
    domain: &'a str,
    name: &'a str,
    #[serde(rename = "type")]
    record_type: &'a str,
    content: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    ttl: Option<u64>,
}

/// Request body for dns.delete action.
#[derive(Debug, Serialize)]
struct DeleteRequest<'a> {
    action: &'static str,
    domain: &'a str,
    id: &'a str,
}

/// Configuration for the Namecrane API client.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// API key.
    pub api_key: String,
    /// The domain this client will manage.
    pub domain: String,
    /// API environment (sandbox or production).
    pub environment: Environment,
}

impl ClientConfig {
    /// Creates a new client configuration.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Your Namecrane API key
    /// * `domain` - The domain to manage
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

    /// Sends a request to the API and handles common error cases.
    async fn request<T: Serialize>(&self, body: &T) -> Result<ApiResponse, NamecraneError> {
        let response = self
            .http_client
            .post(self.base_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await?;

        // Parse response
        let api_response: ApiResponse = serde_json::from_str(&text).map_err(|e| {
            NamecraneError::Parse(format!("Failed to parse response: {} - {}", e, text))
        })?;

        // Handle error responses
        if !api_response.success {
            let code = api_response.code.unwrap_or(status.as_u16());
            let message = api_response
                .error
                .or(api_response.message)
                .unwrap_or_else(|| "Unknown error".to_string());

            return Err(match code {
                401 => NamecraneError::Unauthorized,
                403 => NamecraneError::Forbidden(message),
                404 => {
                    if message.to_lowercase().contains("domain") {
                        NamecraneError::DomainNotFound
                    } else {
                        NamecraneError::RecordNotFound
                    }
                }
                _ => NamecraneError::Api { message, code },
            });
        }

        Ok(api_response)
    }

    /// Lists all DNS records, optionally filtered by type.
    pub async fn list(&self, record_type: Option<&str>) -> Result<Vec<ApiRecord>, NamecraneError> {
        let request = ListRequest {
            action: "dns.list",
            domain: &self.domain,
            record_type,
            id: None,
        };

        let response = self.request(&request).await?;
        Ok(response.records.unwrap_or_default())
    }

    /// Gets a single record by ID.
    pub async fn get(&self, record_id: &str) -> Result<ApiRecord, NamecraneError> {
        let request = ListRequest {
            action: "dns.list",
            domain: &self.domain,
            record_type: None,
            id: Some(record_id),
        };

        let response = self.request(&request).await?;

        // API may return single record in `record` or in `records` array
        response
            .record
            .or_else(|| response.records.and_then(|r| r.into_iter().next()))
            .ok_or(NamecraneError::RecordNotFound)
    }

    /// Creates a new DNS record and returns its ID.
    pub async fn create(
        &self,
        name: &str,
        record_type: &str,
        content: &str,
        ttl: Option<u64>,
    ) -> Result<String, NamecraneError> {
        let request = CreateRequest {
            action: "dns.create",
            domain: &self.domain,
            name,
            record_type,
            content,
            ttl,
        };

        let response = self.request(&request).await?;
        response
            .id
            .ok_or_else(|| NamecraneError::Parse("No record ID in create response".to_string()))
    }

    /// Deletes a DNS record by ID.
    pub async fn delete(&self, record_id: &str) -> Result<(), NamecraneError> {
        let request = DeleteRequest {
            action: "dns.delete",
            domain: &self.domain,
            id: record_id,
        };

        self.request(&request).await?;
        Ok(())
    }
}
