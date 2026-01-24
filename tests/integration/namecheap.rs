//! Integration tests for Namecheap DNS provider.
//!
//! These tests require valid Namecheap credentials and are ignored by default.
//! To run them:
//!
//! 1. Create a `.env` file in the project root (see `.env.example`)
//!
//! 2. Whitelist your IP in Namecheap dashboard
//!
//! 3. Run with: `cargo test --features namecheap -- --ignored`
//!
//! # Environment Variables
//!
//! | Variable | Required | Description |
//! |----------|----------|-------------|
//! | `NAMECHEAP_API_USER` | Yes | Namecheap API username |
//! | `NAMECHEAP_API_KEY` | Yes | Namecheap API key |
//! | `NAMECHEAP_CLIENT_IP` | Yes | Your whitelisted IP address |
//! | `NAMECHEAP_ENVIRONMENT` | No | "sandbox" or "production" (default: sandbox) |
//! | `NAMECHEAP_TEST_DOMAIN` | Yes* | Domain to use for record tests |
//! | `NAMECHEAP_TEST_SUBDOMAIN` | No | Subdomain prefix for test records |
//!
//! *Required for record manipulation tests
//!
//! # Important Notes
//!
//! - Namecheap requires IP whitelisting. Make sure your `NAMECHEAP_CLIENT_IP`
//!   matches your actual public IP and is whitelisted in the dashboard.
//! - The `setHosts` API replaces ALL records. This provider handles this by
//!   fetching existing records before modifications.
//! - Sandbox and production use different credentials and domains.

use manydns::namecheap::{ClientConfig, NamecheapProvider};
use manydns::types::Environment;
use manydns::{CreateRecord, DeleteRecord, Provider, RecordData, Zone};
use std::env;

/// Test configuration loaded from environment.
struct TestConfig {
    provider: NamecheapProvider,
    /// The domain to test with (e.g., "example.com")
    domain: String,
    /// Subdomain prefix for test records (e.g., "namecheap-api-test")
    subdomain: String,
}

impl TestConfig {
    /// Full test host for a given record type (e.g., "a.namecheap-api-test" for A records)
    fn test_host(&self, record_type: &str) -> String {
        format!("{}.{}", record_type.to_lowercase(), self.subdomain)
    }
}

/// Helper to load credentials from environment.
/// Returns None if credentials are not available.
fn get_test_provider() -> Option<NamecheapProvider> {
    // Load .env file if present (ignore errors if file doesn't exist)
    let _ = dotenvy::dotenv();

    let api_user = env::var("NAMECHEAP_API_USER").ok()?;
    let api_key = env::var("NAMECHEAP_API_KEY").ok()?;
    let client_ip = env::var("NAMECHEAP_CLIENT_IP").ok()?;
    let environment = match env::var("NAMECHEAP_ENVIRONMENT")
        .unwrap_or_else(|_| "sandbox".to_string())
        .as_str()
    {
        "production" => Environment::Production,
        _ => Environment::Sandbox,
    };

    let config = ClientConfig::new(api_user, api_key, client_ip, environment);
    NamecheapProvider::new(config).ok()
}

/// Helper to load full test configuration including domain.
fn get_test_config() -> Option<TestConfig> {
    let provider = get_test_provider()?;
    let domain = env::var("NAMECHEAP_TEST_DOMAIN").ok()?;
    let subdomain =
        env::var("NAMECHEAP_TEST_SUBDOMAIN").unwrap_or_else(|_| "namecheap-api-test".to_string());

    Some(TestConfig {
        provider,
        domain,
        subdomain,
    })
}

/// Helper to get the test zone.
async fn get_test_zone(config: &TestConfig) -> impl CreateRecord + DeleteRecord + '_ {
    config
        .provider
        .get_zone(&config.domain)
        .await
        .unwrap_or_else(|_| {
            panic!(
                "Failed to get test domain '{}' - is it using Namecheap DNS?",
                config.domain
            )
        })
}

/// Clean up any existing test records for a given host.
async fn cleanup_test_records<Z: Zone + DeleteRecord>(zone: &Z, host: &str, domain: &str) {
    // Namecheap uses short names (not FQDN), so we need to construct the full host
    let full_host = format!("{}.{}", host, domain);

    let records = zone.list_records().await.unwrap_or_default();
    for record in records {
        if record.host == full_host {
            println!(
                "  Cleaning up existing record: {} (ID: {})",
                record.host, record.id
            );
            let _ = zone.delete_record(&record.id).await;
        }
    }
}

// =============================================================================
// Basic Provider Tests
// =============================================================================

/// Test that we can authenticate and access a zone.
#[tokio::test]
#[ignore = "requires NAMECHEAP credentials and NAMECHEAP_TEST_DOMAIN in .env"]
async fn test_get_zone() {
    let config =
        get_test_config().expect("NAMECHEAP credentials not found. Set credentials in .env");

    let result = config.provider.get_zone(&config.domain).await;

    match result {
        Ok(zone) => {
            println!(
                "Successfully accessed zone: {} (ID: {})",
                zone.domain(),
                zone.id()
            );
        }
        Err(e) => {
            panic!("Failed to get zone: {:?}", e);
        }
    }
}

/// Test that authentication failure is handled properly.
#[tokio::test]
async fn test_invalid_credentials() {
    let config = ClientConfig::new(
        "invalid_user",
        "invalid_api_key",
        "127.0.0.1",
        Environment::Sandbox,
    );
    let provider = NamecheapProvider::new(config).expect("Client creation should succeed");

    let result = provider.get_zone("example.com").await;

    // Should fail with unauthorized or similar error
    assert!(result.is_err(), "Expected error with invalid credentials");
    println!(
        "Correctly got error for invalid credentials: {:?}",
        result.err()
    );
}

/// Test that accessing a non-existent domain returns an error.
#[tokio::test]
#[ignore = "requires NAMECHEAP credentials in .env"]
async fn test_nonexistent_domain() {
    let provider = get_test_provider().expect("NAMECHEAP credentials not found");

    // Try to access a domain that doesn't exist or isn't ours
    let result = provider
        .get_zone("this-domain-definitely-does-not-exist-12345.com")
        .await;

    assert!(
        result.is_err(),
        "Expected error accessing nonexistent domain"
    );
    println!(
        "Correctly got error for nonexistent domain: {:?}",
        result.err()
    );
}

// =============================================================================
// Record Listing Tests
// =============================================================================

/// Test listing records in a zone.
#[tokio::test]
#[ignore = "requires NAMECHEAP credentials and NAMECHEAP_TEST_DOMAIN in .env"]
async fn test_list_records() {
    let config = get_test_config().expect("Test configuration not found");
    let zone = get_test_zone(&config).await;

    let records = zone.list_records().await.expect("Failed to list records");

    println!("Found {} records in {}", records.len(), config.domain);
    for record in &records {
        println!(
            "  - {} {} {:?} (TTL: {}, ID: {})",
            record.host,
            record.data.get_type(),
            record.data.get_value(),
            record.ttl,
            record.id
        );
    }
}

// =============================================================================
// A Record Tests
// =============================================================================

/// Test creating and deleting A records.
#[tokio::test]
#[ignore = "requires NAMECHEAP credentials and NAMECHEAP_TEST_DOMAIN in .env"]
async fn test_a_record_crud() {
    let config = get_test_config().expect("Test configuration not found");
    let zone = get_test_zone(&config).await;
    let host = config.test_host("a");

    println!("Testing A record CRUD for {}.{}", host, config.domain);

    // Cleanup any existing test records
    cleanup_test_records(&zone, &host, &config.domain).await;

    // Create A record
    let ip: std::net::Ipv4Addr = "192.0.2.1".parse().unwrap(); // TEST-NET-1
    let data = RecordData::A(ip);

    println!("  Creating A record: {} -> {}", host, ip);
    let created = zone
        .create_record(&host, &data, 1800)
        .await
        .expect("Failed to create A record");

    assert_eq!(created.data, data);
    println!(
        "  Created record with ID: {}, host: {}",
        created.id, created.host
    );

    // Verify record exists by listing
    let records = zone.list_records().await.expect("Failed to list records");
    let found = records.iter().find(|r| r.id == created.id);
    assert!(found.is_some(), "Created record not found in list");

    // Delete the record
    println!("  Deleting record: {}", created.id);
    zone.delete_record(&created.id)
        .await
        .expect("Failed to delete A record");

    // Verify record is deleted
    let records = zone.list_records().await.expect("Failed to list records");
    let found = records.iter().find(|r| r.id == created.id);
    assert!(found.is_none(), "Record still exists after deletion");

    println!("  A record CRUD test passed!");
}

// =============================================================================
// AAAA Record Tests
// =============================================================================

/// Test creating and deleting AAAA records.
#[tokio::test]
#[ignore = "requires NAMECHEAP credentials and NAMECHEAP_TEST_DOMAIN in .env"]
async fn test_aaaa_record_crud() {
    let config = get_test_config().expect("Test configuration not found");
    let zone = get_test_zone(&config).await;
    let host = config.test_host("aaaa");

    println!("Testing AAAA record CRUD for {}.{}", host, config.domain);

    // Cleanup any existing test records
    cleanup_test_records(&zone, &host, &config.domain).await;

    // Create AAAA record
    let ip: std::net::Ipv6Addr = "2001:db8::1".parse().unwrap(); // Documentation prefix
    let data = RecordData::AAAA(ip);

    println!("  Creating AAAA record: {} -> {}", host, ip);
    let created = zone
        .create_record(&host, &data, 1800)
        .await
        .expect("Failed to create AAAA record");

    assert_eq!(created.data, data);
    println!("  Created record with ID: {}", created.id);

    // Delete the record
    zone.delete_record(&created.id)
        .await
        .expect("Failed to delete AAAA record");

    println!("  AAAA record CRUD test passed!");
}

// =============================================================================
// CNAME Record Tests
// =============================================================================

/// Test creating and deleting CNAME records.
#[tokio::test]
#[ignore = "requires NAMECHEAP credentials and NAMECHEAP_TEST_DOMAIN in .env"]
async fn test_cname_record_crud() {
    let config = get_test_config().expect("Test configuration not found");
    let zone = get_test_zone(&config).await;
    let host = config.test_host("cname");

    println!("Testing CNAME record CRUD for {}.{}", host, config.domain);

    cleanup_test_records(&zone, &host, &config.domain).await;

    let data = RecordData::CNAME("example.com.".to_string());

    println!("  Creating CNAME record: {} -> example.com.", host);
    let created = zone
        .create_record(&host, &data, 1800)
        .await
        .expect("Failed to create CNAME record");

    println!("  Created record with ID: {}", created.id);

    zone.delete_record(&created.id)
        .await
        .expect("Failed to delete CNAME record");

    println!("  CNAME record CRUD test passed!");
}

// =============================================================================
// TXT Record Tests
// =============================================================================

/// Test creating and deleting TXT records.
#[tokio::test]
#[ignore = "requires NAMECHEAP credentials and NAMECHEAP_TEST_DOMAIN in .env"]
async fn test_txt_record_crud() {
    let config = get_test_config().expect("Test configuration not found");
    let zone = get_test_zone(&config).await;
    let host = config.test_host("txt");

    println!("Testing TXT record CRUD for {}.{}", host, config.domain);

    cleanup_test_records(&zone, &host, &config.domain).await;

    let data = RecordData::TXT("v=manydns test record".to_string());

    println!("  Creating TXT record: {} -> {:?}", host, data);
    let created = zone
        .create_record(&host, &data, 1800)
        .await
        .expect("Failed to create TXT record");

    println!("  Created record with ID: {}", created.id);

    zone.delete_record(&created.id)
        .await
        .expect("Failed to delete TXT record");

    println!("  TXT record CRUD test passed!");
}

// =============================================================================
// MX Record Tests
// =============================================================================

/// Test creating and deleting MX records.
///
/// **Note**: Namecheap requires the "Mail Settings" to be configured (e.g., "Custom MX")
/// before MX records can be managed via the DNS API. If Mail Settings is set to
/// "No Email Service", MX record operations will silently fail.
#[tokio::test]
#[ignore = "requires NAMECHEAP credentials and NAMECHEAP_TEST_DOMAIN in .env"]
async fn test_mx_record_crud() {
    let config = get_test_config().expect("Test configuration not found");
    let zone = get_test_zone(&config).await;
    let host = config.test_host("mx");

    println!("Testing MX record CRUD for {}.{}", host, config.domain);

    cleanup_test_records(&zone, &host, &config.domain).await;

    let data = RecordData::MX {
        priority: 10,
        mail_server: "mail.example.com.".to_string(),
    };

    println!("  Creating MX record: {} -> 10 mail.example.com.", host);
    let created = zone
        .create_record(&host, &data, 1800)
        .await
        .expect("Failed to create MX record - ensure Mail Settings is configured in Namecheap");

    println!("  Created record with ID: {}", created.id);

    zone.delete_record(&created.id)
        .await
        .expect("Failed to delete MX record");

    println!("  MX record CRUD test passed!");
}

// =============================================================================
// Error Handling Tests
// =============================================================================

/// Test getting a non-existent record returns NotFound.
#[tokio::test]
#[ignore = "requires NAMECHEAP credentials and NAMECHEAP_TEST_DOMAIN in .env"]
async fn test_get_nonexistent_record() {
    let config = get_test_config().expect("Test configuration not found");
    let zone = get_test_zone(&config).await;

    let result = zone.get_record("nonexistent-record-id-99999999").await;

    assert!(result.is_err(), "Expected error for nonexistent record");
    println!(
        "Correctly got error for nonexistent record: {:?}",
        result.err()
    );
}

/// Test deleting a non-existent record returns NotFound.
#[tokio::test]
#[ignore = "requires NAMECHEAP credentials and NAMECHEAP_TEST_DOMAIN in .env"]
async fn test_delete_nonexistent_record() {
    let config = get_test_config().expect("Test configuration not found");
    let zone = get_test_zone(&config).await;

    let result = zone.delete_record("nonexistent-record-id-99999999").await;

    assert!(
        result.is_err(),
        "Expected error for deleting nonexistent record"
    );
    println!(
        "Correctly got error for deleting nonexistent record: {:?}",
        result.err()
    );
}
