//! Integration tests for Hetzner DNS provider.
//!
//! These tests require valid Hetzner credentials and are ignored by default.
//! To run them:
//!
//! 1. Create a `.env` file in the project root (see `.env.example`)
//!
//! 2. Run with: `cargo test --features hetzner -- --ignored`
//!
//! # Environment Variables
//!
//! | Variable | Required | Description |
//! |----------|----------|-------------|
//! | `HETZNER_API_TOKEN` | Yes | Hetzner DNS API token |
//! | `HETZNER_TEST_DOMAIN` | Yes* | Domain to use for record tests |
//! | `HETZNER_TEST_SUBDOMAIN` | No | Subdomain prefix for test records |
//!
//! *Required for record manipulation tests
//!
//! # Hetzner-Specific Features
//!
//! Unlike other providers, Hetzner supports:
//! - Creating zones via API (`CreateZone` trait)
//! - Deleting zones via API (`DeleteZone` trait)
//! - Zone verification status tracking

#![cfg(feature = "hetzner")]

use libdns::hetzner::HetznerProvider;
use libdns::{CreateRecord, CreateZone, DeleteRecord, DeleteZone, Provider, RecordData, Zone};
use std::env;

/// Test configuration loaded from environment.
struct TestConfig {
    provider: HetznerProvider,
    /// The domain to test with (e.g., "example.com")
    domain: String,
    /// Subdomain prefix for test records (e.g., "hetzner-api-test")
    subdomain: String,
}

impl TestConfig {
    /// Full test host for a given record type (e.g., "a.hetzner-api-test" for A records)
    fn test_host(&self, record_type: &str) -> String {
        format!("{}.{}", record_type.to_lowercase(), self.subdomain)
    }
}

/// Helper to load credentials from environment.
/// Returns None if credentials are not available.
fn get_test_provider() -> Option<HetznerProvider> {
    // Load .env file if present (ignore errors if file doesn't exist)
    let _ = dotenvy::dotenv();

    let api_token = env::var("HETZNER_API_TOKEN").ok()?;

    HetznerProvider::new(&api_token).ok()
}

/// Helper to load full test configuration including domain.
fn get_test_config() -> Option<TestConfig> {
    let provider = get_test_provider()?;
    let domain = env::var("HETZNER_TEST_DOMAIN").ok()?;
    let subdomain =
        env::var("HETZNER_TEST_SUBDOMAIN").unwrap_or_else(|_| "hetzner-api-test".to_string());

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
#[ignore = "requires HETZNER credentials in .env"]
async fn test_list_zones() {
    let provider =
        get_test_provider().expect("HETZNER credentials not found. Set HETZNER_API_TOKEN in .env");

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

/// Test getting a specific zone by domain.
#[tokio::test]
#[ignore = "requires HETZNER credentials and HETZNER_TEST_DOMAIN in .env"]
async fn test_get_zone() {
    let config = get_test_config().expect("Test configuration not found");

    let result = config.provider.get_zone(&config.domain).await;

    match result {
        Ok(zone) => {
            println!("Found zone: {} (ID: {})", zone.domain(), zone.id());
            assert_eq!(zone.domain(), config.domain);
        }
        Err(e) => {
            panic!("Failed to get zone '{}': {:?}", config.domain, e);
        }
    }
}

/// Test that authentication failure is handled properly.
#[tokio::test]
async fn test_invalid_credentials() {
    let provider =
        HetznerProvider::new("invalid_api_token").expect("Client creation should succeed");

    let result = provider.list_zones().await;

    // Should fail with unauthorized or similar error
    assert!(result.is_err(), "Expected error with invalid credentials");
}

/// Test that accessing a zone without permissions returns an error.
#[tokio::test]
#[ignore = "requires HETZNER credentials in .env"]
async fn test_permission_denied_zone() {
    let provider = get_test_provider().expect("HETZNER credentials not found");

    // Try to access example.org which we don't have permissions for
    let result = provider.get_zone("example.org").await;

    assert!(result.is_err(), "Expected error accessing example.org");
    println!(
        "Correctly got error for unauthorized zone: {:?}",
        result.err()
    );
}

// =============================================================================
// Record Listing Tests
// =============================================================================

/// Test listing records in a zone.
#[tokio::test]
#[ignore = "requires HETZNER credentials and HETZNER_TEST_DOMAIN in .env"]
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
#[ignore = "requires HETZNER credentials and HETZNER_TEST_DOMAIN in .env"]
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
        .create_record(&host, &data, 300)
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
#[ignore = "requires HETZNER credentials and HETZNER_TEST_DOMAIN in .env"]
async fn test_aaaa_record_crud() {
    let config = get_test_config().expect("Test configuration not found");
    let zone = get_test_zone(&config).await;
    let host = config.test_host("aaaa");

    println!("Testing AAAA record CRUD for {}.{}", host, config.domain);

    cleanup_test_records(&zone, &host).await;

    // Create AAAA record with a well-known IPv6 address
    let ip: std::net::Ipv6Addr = "2001:db8::1".parse().unwrap(); // Documentation prefix
    let data = RecordData::AAAA(ip);

    println!("  Creating AAAA record: {} -> {}", host, ip);
    let created = zone
        .create_record(&host, &data, 300)
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
#[ignore = "requires HETZNER credentials and HETZNER_TEST_DOMAIN in .env"]
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
        .create_record(&host, &data, 300)
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
#[ignore = "requires HETZNER credentials and HETZNER_TEST_DOMAIN in .env"]
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
        .create_record(&host, &data, 300)
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
#[ignore = "requires HETZNER credentials and HETZNER_TEST_DOMAIN in .env"]
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
        .create_record(&host, &data, 300)
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
// NS Record Tests
// =============================================================================

/// Test creating and deleting NS records.
#[tokio::test]
#[ignore = "requires HETZNER credentials and HETZNER_TEST_DOMAIN in .env"]
async fn test_ns_record_crud() {
    let config = get_test_config().expect("Test configuration not found");
    let zone = get_test_zone(&config).await;
    let host = config.test_host("ns");

    println!("Testing NS record CRUD for {}.{}", host, config.domain);

    cleanup_test_records(&zone, &host).await;

    // Create NS record (delegating subdomain to another nameserver)
    let ns_server = "ns1.example.com";
    let data = RecordData::NS(ns_server.to_string());

    println!("  Creating NS record: {} -> {}", host, ns_server);
    let created = zone
        .create_record(&host, &data, 300)
        .await
        .expect("Failed to create NS record");

    assert_eq!(created.host, host);
    println!("  Created with ID: {}", created.id);

    // Delete it
    zone.delete_record(&created.id)
        .await
        .expect("Failed to delete NS record");
    println!("  Deleted record");
}

// =============================================================================
// SRV Record Tests
// =============================================================================

/// Test creating and deleting SRV records.
#[tokio::test]
#[ignore = "requires HETZNER credentials and HETZNER_TEST_DOMAIN in .env"]
async fn test_srv_record_crud() {
    let config = get_test_config().expect("Test configuration not found");
    let zone = get_test_zone(&config).await;
    // SRV records have special naming: _service._proto.name
    let host = format!("_test._tcp.{}", config.subdomain);

    println!("Testing SRV record CRUD for {}.{}", host, config.domain);

    cleanup_test_records(&zone, &host).await;

    // Create SRV record
    let target = format!("server.{}", config.domain);
    let data = RecordData::SRV {
        priority: 10,
        weight: 5,
        port: 8080,
        target: target.clone(),
    };

    println!(
        "  Creating SRV record: {} -> {} (pri=10, weight=5, port=8080)",
        host, target
    );
    let created = zone
        .create_record(&host, &data, 300)
        .await
        .expect("Failed to create SRV record");

    assert_eq!(created.host, host);
    println!("  Created with ID: {}", created.id);

    // Delete it
    zone.delete_record(&created.id)
        .await
        .expect("Failed to delete SRV record");
    println!("  Deleted record");
}

// =============================================================================
// CAA Record Tests (using Other variant)
// =============================================================================

/// Test creating and deleting CAA records using the Other variant.
#[tokio::test]
#[ignore = "requires HETZNER credentials and HETZNER_TEST_DOMAIN in .env"]
async fn test_caa_record_crud() {
    let config = get_test_config().expect("Test configuration not found");
    let zone = get_test_zone(&config).await;
    let host = config.test_host("caa");

    println!("Testing CAA record CRUD for {}.{}", host, config.domain);

    cleanup_test_records(&zone, &host).await;

    // Create CAA record using Other variant
    // CAA format: <flags> <tag> "<value>"
    let data = RecordData::Other {
        typ: "CAA".to_string(),
        value: "0 issue \"letsencrypt.org\"".to_string(),
    };

    println!("  Creating CAA record: {} -> 0 issue letsencrypt.org", host);
    let created = zone
        .create_record(&host, &data, 300)
        .await
        .expect("Failed to create CAA record");

    assert_eq!(created.host, host);
    println!("  Created with ID: {}", created.id);

    // Delete it
    zone.delete_record(&created.id)
        .await
        .expect("Failed to delete CAA record");
    println!("  Deleted record");
}

// =============================================================================
// Zone Management Tests (Hetzner-specific)
// =============================================================================

/// Test that we can create and delete zones.
/// WARNING: This test creates real zones - use with caution!
#[tokio::test]
#[ignore = "requires HETZNER credentials - creates real zones"]
async fn test_zone_create_delete() {
    let provider =
        get_test_provider().expect("HETZNER credentials not found. Set HETZNER_API_TOKEN in .env");

    // Use a unique test domain that won't conflict
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let test_domain = format!("libdns-test-{}.example", timestamp);

    println!("Creating zone: {}", test_domain);

    // Create zone
    let zone = provider
        .create_zone(&test_domain)
        .await
        .expect("Failed to create zone");

    println!("  Created zone: {} (ID: {})", zone.domain(), zone.id());
    assert_eq!(zone.domain(), test_domain);

    // Delete zone
    provider
        .delete_zone(zone.id())
        .await
        .expect("Failed to delete zone");

    println!("  Deleted zone");

    // Verify it's gone
    let result = provider.get_zone(&test_domain).await;
    assert!(result.is_err(), "Zone should be deleted");
    println!("  Verified zone is deleted");
}

// =============================================================================
// Error Handling Tests
// =============================================================================

/// Test that getting a non-existent record returns NotFound.
#[tokio::test]
#[ignore = "requires HETZNER credentials and HETZNER_TEST_DOMAIN in .env"]
async fn test_get_nonexistent_record() {
    let config = get_test_config().expect("Test configuration not found");
    let zone = get_test_zone(&config).await;

    // Use a fake record ID that doesn't exist
    let result = zone.get_record("nonexistent-record-id-12345").await;

    assert!(result.is_err(), "Expected error for non-existent record");
    println!(
        "Correctly got error for non-existent record: {:?}",
        result.err()
    );
}

/// Test that deleting a non-existent record returns an error.
#[tokio::test]
#[ignore = "requires HETZNER credentials and HETZNER_TEST_DOMAIN in .env"]
async fn test_delete_nonexistent_record() {
    let config = get_test_config().expect("Test configuration not found");
    let zone = get_test_zone(&config).await;

    // Use a fake record ID that doesn't exist
    let result = zone.delete_record("nonexistent-record-id-12345").await;

    assert!(result.is_err(), "Expected error for non-existent record");
    println!(
        "Correctly got error for deleting non-existent record: {:?}",
        result.err()
    );
}

/// Test that deleting a non-existent zone returns an error.
#[tokio::test]
#[ignore = "requires HETZNER credentials in .env"]
async fn test_delete_nonexistent_zone() {
    let provider = get_test_provider().expect("HETZNER credentials not found");

    let result = provider.delete_zone("nonexistent-zone-id-12345").await;

    assert!(result.is_err(), "Expected error for non-existent zone");
    println!(
        "Correctly got error for deleting non-existent zone: {:?}",
        result.err()
    );
}
