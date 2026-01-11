//! Integration tests for Tencent Cloud DNSPod provider.
//!
//! These tests require valid Tencent Cloud credentials and are ignored by default.
//! To run them:
//!
//! 1. Create a `.env` file in the project root (see `.env.example`)
//!
//! 2. Run with: `cargo test --features tencent -- --ignored`
//!
//! # Environment Variables
//!
//! | Variable | Required | Description |
//! |----------|----------|-------------|
//! | `TENCENT_SECRET_ID` | Yes | Tencent Cloud SecretId |
//! | `TENCENT_SECRET_KEY` | Yes | Tencent Cloud SecretKey |
//! | `TENCENT_TEST_DOMAIN` | Yes* | Domain to use for record tests |
//! | `TENCENT_TEST_SUBDOMAIN` | No | Subdomain prefix for test records |
//!
//! *Required for record manipulation tests

#![cfg(feature = "tencent")]

use libdns::tencent::TencentProvider;
use libdns::{CreateRecord, DeleteRecord, Provider, RecordData, Zone};
use std::env;

/// Test configuration loaded from environment.
struct TestConfig {
    provider: TencentProvider,
    /// The domain to test with (e.g., "example.com")
    domain: String,
    /// Subdomain prefix for test records (e.g., "tencent-api-test")
    subdomain: String,
}

impl TestConfig {
    /// Full test host for a given record type (e.g., "a.tencent-api-test" for A records)
    fn test_host(&self, record_type: &str) -> String {
        format!("{}.{}", record_type.to_lowercase(), self.subdomain)
    }
}

/// Helper to load credentials from environment.
/// Returns None if credentials are not available.
fn get_test_provider() -> Option<TencentProvider> {
    // Load .env file if present (ignore errors if file doesn't exist)
    let _ = dotenvy::dotenv();

    let secret_id = env::var("TENCENT_SECRET_ID").ok()?;
    let secret_key = env::var("TENCENT_SECRET_KEY").ok()?;

    TencentProvider::new(&secret_id, &secret_key).ok()
}

/// Helper to load full test configuration including domain.
fn get_test_config() -> Option<TestConfig> {
    let provider = get_test_provider()?;
    let domain = env::var("TENCENT_TEST_DOMAIN").ok()?;
    let subdomain =
        env::var("TENCENT_TEST_SUBDOMAIN").unwrap_or_else(|_| "tencent-api-test".to_string());

    Some(TestConfig {
        provider,
        domain,
        subdomain,
    })
}

/// Helper to get the test zone.
async fn get_test_zone(config: &TestConfig) -> impl Zone + CreateRecord + DeleteRecord + '_ {
    let zones = config
        .provider
        .list_zones()
        .await
        .expect("Failed to list zones");

    zones
        .into_iter()
        .find(|z| z.domain() == config.domain)
        .expect(&format!(
            "Test domain '{}' not found in account",
            config.domain
        ))
}

/// Clean up any existing test records for a given host.
async fn cleanup_test_records<Z: Zone + DeleteRecord>(zone: &Z, host: &str) {
    let records = zone.list_records().await.unwrap_or_default();
    for record in records {
        if record.host == host {
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

/// Test that we can authenticate and list zones.
#[tokio::test]
#[ignore = "requires TENCENT credentials in .env"]
async fn test_list_zones() {
    let provider = get_test_provider().expect(
        "TENCENT credentials not found. Set TENCENT_SECRET_ID and TENCENT_SECRET_KEY in .env",
    );

    let result = provider.list_zones().await;

    match result {
        Ok(zones) => {
            println!("Found {} zones", zones.len());
            for zone in &zones {
                println!("  - {} (ID: {})", zone.domain(), zone.id());
            }
            assert!(!zones.is_empty(), "Expected at least one zone");
        }
        Err(e) => {
            panic!("Failed to list zones: {:?}", e);
        }
    }
}

/// Test that authentication failure is handled properly.
#[tokio::test]
async fn test_invalid_credentials() {
    let provider = TencentProvider::new("invalid_secret_id", "invalid_secret_key")
        .expect("Client creation should succeed");

    let result = provider.list_zones().await;

    // Should fail with unauthorized or similar error
    assert!(result.is_err(), "Expected error with invalid credentials");
}

// =============================================================================
// Record Listing Tests
// =============================================================================

/// Test listing records in a zone.
#[tokio::test]
#[ignore = "requires TENCENT credentials and TENCENT_TEST_DOMAIN in .env"]
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
#[ignore = "requires TENCENT credentials and TENCENT_TEST_DOMAIN in .env"]
async fn test_a_record_crud() {
    let config = get_test_config().expect("Test configuration not found");
    let zone = get_test_zone(&config).await;
    let host = config.test_host("a");

    println!("Testing A record CRUD for {}.{}", host, config.domain);

    // Cleanup any existing test records
    cleanup_test_records(&zone, &host).await;

    // Create A record
    let ip: std::net::Ipv4Addr = "192.0.2.1".parse().unwrap(); // TEST-NET-1
    let data = RecordData::A(ip);

    println!("  Creating A record: {} -> {}", host, ip);
    let created = zone
        .create_record(&host, &data, 600)
        .await
        .expect("Failed to create A record");

    assert_eq!(created.host, host);
    assert_eq!(created.data, data);
    println!("  Created with ID: {}", created.id);

    // Verify it exists
    let record = zone
        .get_record(&created.id)
        .await
        .expect("Failed to get A record");
    assert_eq!(record.data, data);
    println!("  Verified record exists");

    // Delete it
    zone.delete_record(&created.id)
        .await
        .expect("Failed to delete A record");
    println!("  Deleted record");

    // Verify it's gone
    let result = zone.get_record(&created.id).await;
    assert!(result.is_err(), "Record should be deleted");
    println!("  Verified record is deleted");
}

// =============================================================================
// AAAA Record Tests
// =============================================================================

/// Test creating and deleting AAAA records.
#[tokio::test]
#[ignore = "requires TENCENT credentials and TENCENT_TEST_DOMAIN in .env"]
async fn test_aaaa_record_crud() {
    let config = get_test_config().expect("Test configuration not found");
    let zone = get_test_zone(&config).await;
    let host = config.test_host("aaaa");

    println!("Testing AAAA record CRUD for {}.{}", host, config.domain);

    cleanup_test_records(&zone, &host).await;

    // Create AAAA record with Cloudflare DNS address
    let ip: std::net::Ipv6Addr = "2606:4700:4700::1111".parse().unwrap();
    let data = RecordData::AAAA(ip);

    println!("  Creating AAAA record: {} -> {}", host, ip);
    let created = zone
        .create_record(&host, &data, 600)
        .await
        .expect("Failed to create AAAA record");

    assert_eq!(created.host, host);
    println!("  Created with ID: {}", created.id);

    // Delete it
    zone.delete_record(&created.id)
        .await
        .expect("Failed to delete AAAA record");
    println!("  Deleted record");
}

// =============================================================================
// CNAME Record Tests
// =============================================================================

/// Test creating and deleting CNAME records.
#[tokio::test]
#[ignore = "requires TENCENT credentials and TENCENT_TEST_DOMAIN in .env"]
async fn test_cname_record_crud() {
    let config = get_test_config().expect("Test configuration not found");
    let zone = get_test_zone(&config).await;
    let host = config.test_host("cname");

    println!("Testing CNAME record CRUD for {}.{}", host, config.domain);

    cleanup_test_records(&zone, &host).await;

    // Create CNAME record
    let target = format!("target.{}", config.domain);
    let data = RecordData::CNAME(target.clone());

    println!("  Creating CNAME record: {} -> {}", host, target);
    let created = zone
        .create_record(&host, &data, 600)
        .await
        .expect("Failed to create CNAME record");

    assert_eq!(created.host, host);
    println!("  Created with ID: {}", created.id);

    // Delete it
    zone.delete_record(&created.id)
        .await
        .expect("Failed to delete CNAME record");
    println!("  Deleted record");
}

// =============================================================================
// MX Record Tests
// =============================================================================

/// Test creating and deleting MX records.
#[tokio::test]
#[ignore = "requires TENCENT credentials and TENCENT_TEST_DOMAIN in .env"]
async fn test_mx_record_crud() {
    let config = get_test_config().expect("Test configuration not found");
    let zone = get_test_zone(&config).await;
    let host = config.test_host("mx");

    println!("Testing MX record CRUD for {}.{}", host, config.domain);

    cleanup_test_records(&zone, &host).await;

    // Create MX record
    let mail_server = format!("mail.{}", config.domain);
    let data = RecordData::MX {
        priority: 10,
        mail_server: mail_server.clone(),
    };

    println!(
        "  Creating MX record: {} -> {} (priority 10)",
        host, mail_server
    );
    let created = zone
        .create_record(&host, &data, 600)
        .await
        .expect("Failed to create MX record");

    assert_eq!(created.host, host);
    println!("  Created with ID: {}", created.id);

    // Delete it
    zone.delete_record(&created.id)
        .await
        .expect("Failed to delete MX record");
    println!("  Deleted record");
}

// =============================================================================
// TXT Record Tests
// =============================================================================

/// Test creating and deleting TXT records.
#[tokio::test]
#[ignore = "requires TENCENT credentials and TENCENT_TEST_DOMAIN in .env"]
async fn test_txt_record_crud() {
    let config = get_test_config().expect("Test configuration not found");
    let zone = get_test_zone(&config).await;
    let host = config.test_host("txt");

    println!("Testing TXT record CRUD for {}.{}", host, config.domain);

    cleanup_test_records(&zone, &host).await;

    // Create TXT record
    let txt_value = "v=libdns-test; test=true";
    let data = RecordData::TXT(txt_value.to_string());

    println!("  Creating TXT record: {} -> \"{}\"", host, txt_value);
    let created = zone
        .create_record(&host, &data, 600)
        .await
        .expect("Failed to create TXT record");

    assert_eq!(created.host, host);
    println!("  Created with ID: {}", created.id);

    // Delete it
    zone.delete_record(&created.id)
        .await
        .expect("Failed to delete TXT record");
    println!("  Deleted record");
}

// =============================================================================
// Error Handling Tests
// =============================================================================

/// Test that getting a non-existent record returns NotFound.
#[tokio::test]
#[ignore = "requires TENCENT credentials and TENCENT_TEST_DOMAIN in .env"]
async fn test_get_nonexistent_record() {
    let config = get_test_config().expect("Test configuration not found");
    let zone = get_test_zone(&config).await;

    let result = zone.get_record("99999999999").await;

    assert!(result.is_err(), "Expected error for non-existent record");
    println!(
        "Correctly got error for non-existent record: {:?}",
        result.err()
    );
}

/// Test that deleting a non-existent record returns an error.
#[tokio::test]
#[ignore = "requires TENCENT credentials and TENCENT_TEST_DOMAIN in .env"]
async fn test_delete_nonexistent_record() {
    let config = get_test_config().expect("Test configuration not found");
    let zone = get_test_zone(&config).await;

    let result = zone.delete_record("99999999999").await;

    assert!(result.is_err(), "Expected error for non-existent record");
    println!(
        "Correctly got error for deleting non-existent record: {:?}",
        result.err()
    );
}
