//! Namecheap API client implementation.
//!
//! This module provides a low-level HTTP client for the Namecheap API.
//! The API uses XML responses and requires GET/POST requests with query parameters.

use std::error::Error;
use std::fmt;

use quick_xml::events::Event;
use quick_xml::Reader;
use reqwest::Client as HttpClient;

use crate::types::Environment;
use crate::HttpClientConfig;

/// Namecheap API endpoints.
const PRODUCTION_API_URL: &str = "https://api.namecheap.com/xml.response";
const SANDBOX_API_URL: &str = "https://api.sandbox.namecheap.com/xml.response";

/// Error returned by the Namecheap API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    /// The error code from Namecheap.
    pub code: String,
    /// The error message.
    pub message: String,
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Namecheap API error {}: {}", self.code, self.message)
    }
}

impl Error for ApiError {}

/// Errors that can occur when using the Namecheap API.
#[derive(Debug)]
pub enum NamecheapError {
    /// HTTP request error.
    Request(reqwest::Error),
    /// API error response.
    Api(ApiError),
    /// XML parsing error.
    Parse(String),
    /// Domain not found or not using Namecheap DNS.
    DomainNotFound,
    /// Unauthorized access.
    Unauthorized,
    /// Rate limited by the API.
    RateLimited,
}

impl fmt::Display for NamecheapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NamecheapError::Request(e) => write!(f, "HTTP request error: {}", e),
            NamecheapError::Api(e) => write!(f, "{}", e),
            NamecheapError::Parse(msg) => write!(f, "XML parse error: {}", msg),
            NamecheapError::DomainNotFound => write!(f, "Domain not found"),
            NamecheapError::Unauthorized => write!(f, "Unauthorized"),
            NamecheapError::RateLimited => write!(f, "Rate limited"),
        }
    }
}

impl Error for NamecheapError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            NamecheapError::Request(e) => Some(e),
            NamecheapError::Api(e) => Some(e),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for NamecheapError {
    fn from(err: reqwest::Error) -> Self {
        NamecheapError::Request(err)
    }
}

/// A DNS host record from Namecheap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRecord {
    /// Unique ID of the host record.
    pub host_id: String,
    /// The hostname/subdomain (e.g., "@", "www", "mail").
    pub name: String,
    /// Record type (A, AAAA, CNAME, MX, TXT, etc.).
    pub record_type: String,
    /// The record value (IP address, hostname, etc.).
    pub address: String,
    /// MX preference (only for MX records).
    pub mx_pref: Option<u16>,
    /// TTL in seconds.
    pub ttl: u64,
}

/// Configuration for the Namecheap API client.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// API username (your Namecheap account username).
    pub api_user: String,
    /// API key from Namecheap.
    pub api_key: String,
    /// Username for command execution (usually same as api_user).
    pub username: String,
    /// Client IP address (must be whitelisted in Namecheap).
    pub client_ip: String,
    /// API environment (sandbox or production).
    pub environment: Environment,
}

impl ClientConfig {
    /// Creates a new client configuration.
    ///
    /// # Arguments
    ///
    /// * `api_user` - Your Namecheap username
    /// * `api_key` - Your Namecheap API key
    /// * `client_ip` - Your whitelisted IP address
    /// * `environment` - Sandbox or Production
    pub fn new(
        api_user: impl Into<String>,
        api_key: impl Into<String>,
        client_ip: impl Into<String>,
        environment: Environment,
    ) -> Self {
        let api_user = api_user.into();
        Self {
            username: api_user.clone(),
            api_user,
            api_key: api_key.into(),
            client_ip: client_ip.into(),
            environment,
        }
    }

    /// Creates a configuration for the sandbox environment.
    pub fn sandbox(
        api_user: impl Into<String>,
        api_key: impl Into<String>,
        client_ip: impl Into<String>,
    ) -> Self {
        Self::new(api_user, api_key, client_ip, Environment::Sandbox)
    }

    /// Creates a configuration for the production environment.
    pub fn production(
        api_user: impl Into<String>,
        api_key: impl Into<String>,
        client_ip: impl Into<String>,
    ) -> Self {
        Self::new(api_user, api_key, client_ip, Environment::Production)
    }

    /// Returns the API base URL for the configured environment.
    pub fn api_url(&self) -> &'static str {
        match self.environment {
            Environment::Production => PRODUCTION_API_URL,
            Environment::Sandbox => SANDBOX_API_URL,
        }
    }
}

/// Namecheap API client.
#[derive(Debug, Clone)]
pub struct Client {
    http_client: HttpClient,
    config: ClientConfig,
}

impl Client {
    /// Creates a new Namecheap API client.
    pub fn new(config: ClientConfig) -> Result<Self, Box<dyn Error + Send + Sync>> {
        Self::with_http_config(config, HttpClientConfig::default())
    }

    /// Creates a new Namecheap API client with custom HTTP configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Namecheap client configuration
    /// * `http_config` - HTTP client configuration for network binding
    pub fn with_http_config(
        config: ClientConfig,
        http_config: HttpClientConfig,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let mut builder = HttpClient::builder().user_agent("manydns-rs/1.0.0");

        if let Some(timeout) = http_config.timeout {
            builder = builder.timeout(timeout);
        }

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

        Ok(Self {
            http_client,
            config,
        })
    }

    /// Returns the configured environment.
    pub fn environment(&self) -> Environment {
        self.config.environment
    }

    /// URL-encode a string for use in query parameters.
    fn url_encode(s: &str) -> String {
        let mut encoded = String::with_capacity(s.len() * 3);
        for byte in s.bytes() {
            match byte {
                // Unreserved characters (RFC 3986)
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    encoded.push(byte as char);
                }
                // Everything else gets percent-encoded
                _ => {
                    encoded.push('%');
                    encoded.push_str(&format!("{:02X}", byte));
                }
            }
        }
        encoded
    }

    /// Build a query string from key-value pairs.
    fn build_query_string(params: &[(&str, &str)]) -> String {
        params
            .iter()
            .map(|(k, v)| format!("{}={}", k, Self::url_encode(v)))
            .collect::<Vec<_>>()
            .join("&")
    }

    /// Makes an API request with the given command and parameters.
    async fn request(
        &self,
        command: &str,
        params: &[(&str, &str)],
    ) -> Result<String, NamecheapError> {
        let mut query_params: Vec<(&str, &str)> = vec![
            ("ApiUser", &self.config.api_user),
            ("ApiKey", &self.config.api_key),
            ("UserName", &self.config.username),
            ("ClientIp", &self.config.client_ip),
            ("Command", command),
        ];
        query_params.extend_from_slice(params);

        // Build URL with query string
        let query_string = Self::build_query_string(&query_params);
        let url = format!("{}?{}", self.config.api_url(), query_string);

        let response = self.http_client.get(&url).send().await?;

        let text = response.text().await?;

        // Check for API errors in the response
        self.check_api_error(&text)?;

        Ok(text)
    }

    /// Checks the XML response for API errors.
    fn check_api_error(&self, xml: &str) -> Result<(), NamecheapError> {
        let mut reader = Reader::from_str(xml);

        let mut in_error = false;
        let mut error_code: Option<String> = None;
        let mut error_message: Option<String> = None;
        let mut is_error_status = false;

        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    let name = e.local_name();
                    if name.as_ref() == b"ApiResponse" {
                        // Check Status attribute
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"Status" && attr.value.as_ref() == b"ERROR" {
                                is_error_status = true;
                            }
                        }
                    } else if name.as_ref() == b"Error" {
                        in_error = true;
                        // Get Number attribute
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"Number" {
                                error_code =
                                    String::from_utf8_lossy(&attr.value).into_owned().into();
                            }
                        }
                    }
                }
                Ok(Event::Text(ref e)) if in_error => {
                    error_message = Some(String::from_utf8_lossy(e.as_ref()).into_owned());
                }
                Ok(Event::End(ref e)) if e.local_name().as_ref() == b"Error" => {
                    in_error = false;
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(NamecheapError::Parse(format!("XML parse error: {}", e)));
                }
                _ => {}
            }
        }

        if is_error_status {
            if let (Some(code), Some(msg)) = (error_code, error_message) {
                // Map known error codes
                match code.as_str() {
                    "1010102" | "1011102" | "1030408" => {
                        return Err(NamecheapError::Unauthorized);
                    }
                    "2019166" | "2016166" => {
                        return Err(NamecheapError::DomainNotFound);
                    }
                    "500000" => {
                        return Err(NamecheapError::RateLimited);
                    }
                    _ => {}
                }

                return Err(NamecheapError::Api(ApiError { code, message: msg }));
            }

            return Err(NamecheapError::Parse(
                "Failed to parse error response".to_string(),
            ));
        }

        Ok(())
    }

    /// Gets DNS host records for a domain.
    ///
    /// # Arguments
    ///
    /// * `sld` - Second-level domain (e.g., "example" for example.com)
    /// * `tld` - Top-level domain (e.g., "com" for example.com)
    pub async fn get_hosts(&self, sld: &str, tld: &str) -> Result<Vec<HostRecord>, NamecheapError> {
        #[cfg(debug_assertions)]
        eprintln!("DEBUG get_hosts: sld='{}', tld='{}'", sld, tld);

        let xml = self
            .request(
                "namecheap.domains.dns.getHosts",
                &[("SLD", sld), ("TLD", tld)],
            )
            .await?;

        #[cfg(debug_assertions)]
        eprintln!("DEBUG get_hosts response:\n{}", &xml);

        // Check if domain is using Namecheap DNS
        if let Some(using_our_dns) =
            get_element_attr(&xml, "DomainDNSGetHostsResult", "IsUsingOurDNS")?
        {
            if using_our_dns != "true" {
                return Err(NamecheapError::Api(ApiError {
                    code: "2030288".to_string(),
                    message: "Domain is not using Namecheap DNS servers".to_string(),
                }));
            }
        }

        // Parse host records
        let records = parse_host_records(&xml)?;
        #[cfg(debug_assertions)]
        eprintln!("DEBUG get_hosts parsed {} records", records.len());
        Ok(records)
    }

    /// Sets DNS host records for a domain.
    ///
    /// **Important**: This replaces ALL existing records. Include all records you want to keep.
    ///
    /// # Arguments
    ///
    /// * `sld` - Second-level domain (e.g., "example" for example.com)
    /// * `tld` - Top-level domain (e.g., "com" for example.com)
    /// * `records` - All host records to set
    pub async fn set_hosts(
        &self,
        sld: &str,
        tld: &str,
        records: &[HostRecord],
    ) -> Result<(), NamecheapError> {
        #[cfg(debug_assertions)]
        eprintln!(
            "DEBUG set_hosts: sld='{}', tld='{}', {} records",
            sld,
            tld,
            records.len()
        );

        let mut params: Vec<(String, String)> = vec![
            ("SLD".to_string(), sld.to_string()),
            ("TLD".to_string(), tld.to_string()),
        ];

        // Add each record with numbered parameters
        for (i, record) in records.iter().enumerate() {
            let n = i + 1;
            #[cfg(debug_assertions)]
            eprintln!(
                "DEBUG set_hosts record {}: name='{}', type='{}', address='{}', mx_pref={:?}",
                n, record.name, record.record_type, record.address, record.mx_pref
            );
            params.push((format!("HostName{}", n), record.name.clone()));
            params.push((format!("RecordType{}", n), record.record_type.clone()));
            params.push((format!("Address{}", n), record.address.clone()));
            params.push((format!("TTL{}", n), record.ttl.to_string()));

            if let Some(mx_pref) = record.mx_pref {
                params.push((format!("MXPref{}", n), mx_pref.to_string()));
            }
        }

        // Convert to slice of tuples with string references
        let param_refs: Vec<(&str, &str)> = params
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        let response = self
            .request("namecheap.domains.dns.setHosts", &param_refs)
            .await?;

        #[cfg(debug_assertions)]
        eprintln!(
            "DEBUG set_hosts response (first 1000 chars): {}",
            &response[..response.len().min(1000)]
        );

        Ok(())
    }
}

/// Parses host records from XML response using quick-xml.
///
/// This is useful for custom parsing of Namecheap API responses.
pub fn parse_host_records(xml: &str) -> Result<Vec<HostRecord>, NamecheapError> {
    let mut reader = Reader::from_str(xml);
    let mut records = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                // Check for "host" (case-insensitive)
                let local_name = e.local_name();
                if local_name.as_ref().eq_ignore_ascii_case(b"host") {
                    let mut host_id = String::new();
                    let mut name = String::new();
                    let mut record_type = String::new();
                    let mut address = String::new();
                    let mut ttl: u64 = 1800;
                    let mut mx_pref: Option<u16> = None;

                    for attr in e.attributes().flatten() {
                        let value = String::from_utf8_lossy(&attr.value).into_owned();
                        match attr.key.as_ref() {
                            b"HostId" => host_id = value,
                            b"Name" => name = value,
                            b"Type" => record_type = value,
                            b"Address" => address = value,
                            b"TTL" => ttl = value.parse().unwrap_or(1800),
                            b"MXPref" => mx_pref = value.parse().ok(),
                            _ => {}
                        }
                    }

                    records.push(HostRecord {
                        host_id,
                        name,
                        record_type,
                        address,
                        mx_pref,
                        ttl,
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(NamecheapError::Parse(format!("XML parse error: {}", e)));
            }
            _ => {}
        }
    }

    Ok(records)
}

/// Gets an attribute value from a specific XML element.
///
/// This is useful for extracting data from Namecheap API responses.
pub fn get_element_attr(
    xml: &str,
    tag: &str,
    attr: &str,
) -> Result<Option<String>, NamecheapError> {
    let mut reader = Reader::from_str(xml);
    let tag_bytes = tag.as_bytes();
    let attr_bytes = attr.as_bytes();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                if e.local_name().as_ref() == tag_bytes {
                    for a in e.attributes().flatten() {
                        if a.key.as_ref() == attr_bytes {
                            return Ok(Some(String::from_utf8_lossy(&a.value).into_owned()));
                        }
                    }
                    return Ok(None);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(NamecheapError::Parse(format!("XML parse error: {}", e)));
            }
            _ => {}
        }
    }

    Ok(None)
}
