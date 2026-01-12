//! Mock-based tests for Cloudflare provider.
//!
//! These tests use wiremock to simulate API responses without hitting real APIs.

use crate::common::cloudflare::*;
use crate::common::setup_mock_server;

use libdns::cloudflare::CloudflareProvider;
use libdns::{
    CreateRecord, CreateRecordError, DeleteRecord, DeleteRecordError, Provider, RecordData,
    RetrieveRecordError, RetrieveZoneError, Zone,
};
use proptest::prelude::*;
use serde_json::json;
use std::net::Ipv4Addr;
use wiremock::matchers::{header, method, path, path_regex, query_param};
use wiremock::{Mock, ResponseTemplate};

// =============================================================================
// Zone Tests
// =============================================================================

#[tokio::test]
async fn test_list_zones_success() {
    let server = setup_mock_server().await;

    Mock::given(method("GET"))
        .and(path_regex(r"^/zones$"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_zones_list_response(vec![
                (ZONE_ID_1, "example.com"),
                (ZONE_ID_2, "test.org"),
            ])),
        )
        .mount(&server)
        .await;

    let provider = CloudflareProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zones = provider.list_zones().await.expect("Failed to list zones");
    assert_eq!(zones.len(), 2);
    assert_eq!(zones[0].domain(), "example.com");
    assert_eq!(zones[1].domain(), "test.org");
}

#[tokio::test]
async fn test_list_zones_empty() {
    let server = setup_mock_server().await;

    Mock::given(method("GET"))
        .and(path_regex(r"^/zones$"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_zones_list_response(vec![])))
        .mount(&server)
        .await;

    let provider = CloudflareProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zones = provider.list_zones().await.expect("Failed to list zones");
    assert!(zones.is_empty());
}

#[tokio::test]
async fn test_list_zones_unauthorized() {
    let server = setup_mock_server().await;

    Mock::given(method("GET"))
        .and(path_regex(r"^/zones$"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_json(mock_error_response(10000, "Authentication error")),
        )
        .mount(&server)
        .await;

    let provider = CloudflareProvider::with_base_url("bad-token", &server.uri())
        .expect("Failed to create provider");

    let result = provider.list_zones().await;
    assert!(matches!(result, Err(RetrieveZoneError::Unauthorized)));
}

#[tokio::test]
async fn test_get_zone_by_id_success() {
    let server = setup_mock_server().await;

    Mock::given(method("GET"))
        .and(path(format!("/zones/{}", ZONE_ID_1)))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_zone_response(ZONE_ID_1, "example.com")),
        )
        .mount(&server)
        .await;

    let provider = CloudflareProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zone = provider
        .get_zone(ZONE_ID_1)
        .await
        .expect("Failed to get zone");
    assert_eq!(zone.id(), ZONE_ID_1);
    assert_eq!(zone.domain(), "example.com");
}

#[tokio::test]
async fn test_get_zone_not_found() {
    let server = setup_mock_server().await;

    // For a "not found" zone, we use a name lookup since "nonexistent.com" is not a 32-char hex
    Mock::given(method("GET"))
        .and(path("/zones"))
        .and(query_param("name", "nonexistent.com"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_zones_list_response(vec![])))
        .mount(&server)
        .await;

    let provider = CloudflareProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let result = provider.get_zone("nonexistent.com").await;
    assert!(matches!(result, Err(RetrieveZoneError::NotFound)));
}

#[tokio::test]
async fn test_get_zone_by_name_success() {
    let server = setup_mock_server().await;

    // Query zones by name
    Mock::given(method("GET"))
        .and(path("/zones"))
        .and(query_param("name", "example.com"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(mock_zones_list_response(vec![(ZONE_ID_1, "example.com")])),
        )
        .mount(&server)
        .await;

    let provider = CloudflareProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zone = provider
        .get_zone("example.com")
        .await
        .expect("Failed to get zone");
    assert_eq!(zone.domain(), "example.com");
}

// =============================================================================
// Record Tests
// =============================================================================

#[tokio::test]
async fn test_list_records_success() {
    let server = setup_mock_server().await;

    // Zone lookup by name
    Mock::given(method("GET"))
        .and(path("/zones"))
        .and(query_param("name", "example.com"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(mock_zones_list_response(vec![(ZONE_ID_1, "example.com")])),
        )
        .mount(&server)
        .await;

    // Records list
    Mock::given(method("GET"))
        .and(path(format!("/zones/{}/dns_records", ZONE_ID_1)))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_records_list_response(vec![
                (
                    RECORD_ID_1,
                    ZONE_ID_1,
                    "example.com",
                    "example.com",
                    "A",
                    "192.168.1.1",
                    300,
                ),
                (
                    RECORD_ID_2,
                    ZONE_ID_1,
                    "example.com",
                    "www.example.com",
                    "CNAME",
                    "example.com",
                    300,
                ),
            ])),
        )
        .mount(&server)
        .await;

    let provider = CloudflareProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zone = provider
        .get_zone("example.com")
        .await
        .expect("Failed to get zone");
    let records = zone.list_records().await.expect("Failed to list records");

    assert_eq!(records.len(), 2);
    assert_eq!(records[0].data.get_type(), "A");
    assert_eq!(records[1].data.get_type(), "CNAME");
}

#[tokio::test]
async fn test_get_record_success() {
    let server = setup_mock_server().await;

    // Zone lookup by name
    Mock::given(method("GET"))
        .and(path("/zones"))
        .and(query_param("name", "example.com"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(mock_zones_list_response(vec![(ZONE_ID_1, "example.com")])),
        )
        .mount(&server)
        .await;

    // Record lookup
    Mock::given(method("GET"))
        .and(path(format!(
            "/zones/{}/dns_records/{}",
            ZONE_ID_1, RECORD_ID_1
        )))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_record_response(
                RECORD_ID_1,
                ZONE_ID_1,
                "example.com",
                "example.com",
                "A",
                "192.168.1.1",
                300,
            )),
        )
        .mount(&server)
        .await;

    let provider = CloudflareProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zone = provider
        .get_zone("example.com")
        .await
        .expect("Failed to get zone");
    let record = zone
        .get_record(RECORD_ID_1)
        .await
        .expect("Failed to get record");

    assert_eq!(record.id, RECORD_ID_1);
    assert_eq!(record.data, RecordData::A(Ipv4Addr::new(192, 168, 1, 1)));
}

#[tokio::test]
async fn test_get_record_not_found() {
    let server = setup_mock_server().await;

    // Zone lookup by name
    Mock::given(method("GET"))
        .and(path("/zones"))
        .and(query_param("name", "example.com"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(mock_zones_list_response(vec![(ZONE_ID_1, "example.com")])),
        )
        .mount(&server)
        .await;

    // Record not found - use a valid 32-char hex that doesn't exist
    let nonexistent_record = "00000000000000000000000000000000";
    Mock::given(method("GET"))
        .and(path(format!(
            "/zones/{}/dns_records/{}",
            ZONE_ID_1, nonexistent_record
        )))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_json(mock_error_response(81044, "Record not found")),
        )
        .mount(&server)
        .await;

    let provider = CloudflareProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zone = provider
        .get_zone("example.com")
        .await
        .expect("Failed to get zone");
    let result = zone.get_record(nonexistent_record).await;

    assert!(matches!(result, Err(RetrieveRecordError::NotFound)));
}

#[tokio::test]
async fn test_create_record_success() {
    let server = setup_mock_server().await;

    // Zone lookup by name
    Mock::given(method("GET"))
        .and(path("/zones"))
        .and(query_param("name", "example.com"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(mock_zones_list_response(vec![(ZONE_ID_1, "example.com")])),
        )
        .mount(&server)
        .await;

    // Create record
    Mock::given(method("POST"))
        .and(path(format!("/zones/{}/dns_records", ZONE_ID_1)))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_record_response(
                NEW_RECORD_ID,
                ZONE_ID_1,
                "example.com",
                "test.example.com",
                "A",
                "10.0.0.1",
                300,
            )),
        )
        .mount(&server)
        .await;

    let provider = CloudflareProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zone = provider
        .get_zone("example.com")
        .await
        .expect("Failed to get zone");
    let record = zone
        .create_record("test", &RecordData::A(Ipv4Addr::new(10, 0, 0, 1)), 300)
        .await
        .expect("Failed to create record");

    assert_eq!(record.id, NEW_RECORD_ID);
}

#[tokio::test]
async fn test_create_record_unsupported_type() {
    let server = setup_mock_server().await;

    // Zone lookup
    Mock::given(method("GET"))
        .and(path("/zones"))
        .and(query_param("name", "example.com"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(mock_zones_list_response(vec![(ZONE_ID_1, "example.com")])),
        )
        .mount(&server)
        .await;

    let provider = CloudflareProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zone = provider
        .get_zone("example.com")
        .await
        .expect("Failed to get zone");

    // Try to create an unsupported record type
    let result = zone
        .create_record(
            "test",
            &RecordData::Other {
                typ: "UNSUPPORTED".to_string(),
                value: "value".to_string(),
            },
            300,
        )
        .await;

    assert!(matches!(result, Err(CreateRecordError::UnsupportedType)));
}

#[tokio::test]
async fn test_delete_record_success() {
    let server = setup_mock_server().await;

    // Zone lookup
    Mock::given(method("GET"))
        .and(path("/zones"))
        .and(query_param("name", "example.com"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(mock_zones_list_response(vec![(ZONE_ID_1, "example.com")])),
        )
        .mount(&server)
        .await;

    // Delete record
    Mock::given(method("DELETE"))
        .and(path(format!(
            "/zones/{}/dns_records/{}",
            ZONE_ID_1, RECORD_ID_1
        )))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_delete_response(RECORD_ID_1)))
        .mount(&server)
        .await;

    let provider = CloudflareProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zone = provider
        .get_zone("example.com")
        .await
        .expect("Failed to get zone");
    zone.delete_record(RECORD_ID_1)
        .await
        .expect("Failed to delete record");
}

#[tokio::test]
async fn test_delete_record_not_found() {
    let server = setup_mock_server().await;

    // Zone lookup
    Mock::given(method("GET"))
        .and(path("/zones"))
        .and(query_param("name", "example.com"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(mock_zones_list_response(vec![(ZONE_ID_1, "example.com")])),
        )
        .mount(&server)
        .await;

    // Delete non-existent record
    let nonexistent_record = "00000000000000000000000000000000";
    Mock::given(method("DELETE"))
        .and(path(format!(
            "/zones/{}/dns_records/{}",
            ZONE_ID_1, nonexistent_record
        )))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_json(mock_error_response(81044, "Record not found")),
        )
        .mount(&server)
        .await;

    let provider = CloudflareProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zone = provider
        .get_zone("example.com")
        .await
        .expect("Failed to get zone");
    let result = zone.delete_record(nonexistent_record).await;

    assert!(matches!(result, Err(DeleteRecordError::NotFound)));
}

// =============================================================================
// Property-based Mock Tests
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn test_various_a_records(ip in (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let server = setup_mock_server().await;
            let ip_addr = Ipv4Addr::new(ip.0, ip.1, ip.2, ip.3);

            // Zone lookup by name
            Mock::given(method("GET"))
                .and(path("/zones"))
                .and(query_param("name", "example.com"))
                .and(header("Authorization", "Bearer test-token"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(mock_zones_list_response(vec![(ZONE_ID_1, "example.com")])),
                )
                .mount(&server)
                .await;

            // Record response with the generated IP
            Mock::given(method("GET"))
                .and(path(format!("/zones/{}/dns_records/{}", ZONE_ID_1, RECORD_ID_1)))
                .and(header("Authorization", "Bearer test-token"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(mock_record_response(
                            RECORD_ID_1, ZONE_ID_1, "example.com", "example.com", "A", &ip_addr.to_string(), 300
                        )),
                )
                .mount(&server)
                .await;

            let provider = CloudflareProvider::with_base_url("test-token", &server.uri())
                .expect("Failed to create provider");

            let zone = provider.get_zone("example.com").await.expect("Failed to get zone");
            let record = zone.get_record(RECORD_ID_1).await.expect("Failed to get record");

            prop_assert_eq!(record.data, RecordData::A(ip_addr));
            Ok(())
        }).unwrap();
    }

    #[test]
    fn test_various_ttl_values(ttl in 1u32..=86400u32) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let server = setup_mock_server().await;

            Mock::given(method("GET"))
                .and(path("/zones"))
                .and(query_param("name", "example.com"))
                .and(header("Authorization", "Bearer test-token"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(mock_zones_list_response(vec![(ZONE_ID_1, "example.com")])),
                )
                .mount(&server)
                .await;

            Mock::given(method("GET"))
                .and(path(format!("/zones/{}/dns_records/{}", ZONE_ID_1, RECORD_ID_1)))
                .and(header("Authorization", "Bearer test-token"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(json!({
                            "success": true,
                            "errors": [],
                            "messages": [],
                            "result": {
                                "id": RECORD_ID_1,
                                "zone_id": ZONE_ID_1,
                                "zone_name": "example.com",
                                "name": "example.com",
                                "type": "A",
                                "content": "1.2.3.4",
                                "proxied": false,
                                "ttl": ttl
                            }
                        })),
                )
                .mount(&server)
                .await;

            let provider = CloudflareProvider::with_base_url("test-token", &server.uri())
                .expect("Failed to create provider");

            let zone = provider.get_zone("example.com").await.expect("Failed to get zone");
            let record = zone.get_record(RECORD_ID_1).await.expect("Failed to get record");

            prop_assert_eq!(record.ttl, ttl as u64);
            Ok(())
        }).unwrap();
    }
}
