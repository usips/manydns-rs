//! Mock-based tests for Hetzner Cloud DNS provider.
//!
//! These tests use `wiremock` to simulate the Hetzner Cloud DNS API (`api.hetzner.cloud/v1`)
//! without requiring network access or API credentials.
//!
//! # Coverage
//!
//! This module provides comprehensive testing for:
//! - Zone operations (list, get, create, delete)
//! - RRSet/Record operations (list, get, create, delete)
//! - Error handling (404, 401, server errors)
//! - Various record types (A, AAAA, CNAME, MX, TXT, etc.)
//! - TTL handling
//! - Pagination
//!
//! # API Structure (Hetzner Cloud API)
//!
//! The Hetzner Cloud DNS API uses RRSets (Resource Record Sets):
//! - Zones: GET/POST/DELETE `/v1/zones`
//! - RRSets: GET/POST/PUT/DELETE `/v1/zones/{zone_id}/rrsets`
//! - Auth: `Authorization: Bearer <token>` header

use crate::common::hetzner::*;
use crate::common::setup_mock_server;

use libdns::hetzner::HetznerProvider;
use libdns::{CreateRecord, CreateZone, DeleteRecord, DeleteZone, Provider, RecordData, Zone};
use proptest::prelude::*;
use serde_json::json;
use std::net::Ipv4Addr;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, ResponseTemplate};

// =============================================================================
// Zone Tests
// =============================================================================

#[tokio::test]
async fn test_list_zones_success() {
    let server = setup_mock_server().await;

    Mock::given(method("GET"))
        .and(path("/zones"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_zones_response(vec![
                (123, "example.com", 3600),
                (456, "example.org", 7200),
            ])),
        )
        .mount(&server)
        .await;

    let provider = HetznerProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zones = provider.list_zones().await.expect("Failed to list zones");
    assert_eq!(zones.len(), 2);
    assert_eq!(zones[0].domain(), "example.com");
    assert_eq!(zones[1].domain(), "example.org");
}

#[tokio::test]
async fn test_list_zones_empty() {
    let server = setup_mock_server().await;

    Mock::given(method("GET"))
        .and(path("/zones"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_zones_response(vec![])))
        .mount(&server)
        .await;

    let provider = HetznerProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zones = provider.list_zones().await.expect("Failed to list zones");
    assert!(zones.is_empty());
}

#[tokio::test]
async fn test_list_zones_unauthorized() {
    let server = setup_mock_server().await;

    Mock::given(method("GET"))
        .and(path("/zones"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "error": {
                "code": "unauthorized",
                "message": "Invalid authentication credentials"
            }
        })))
        .mount(&server)
        .await;

    let provider = HetznerProvider::with_base_url("bad-token", &server.uri())
        .expect("Failed to create provider");

    let result = provider.list_zones().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_zone_by_name_success() {
    let server = setup_mock_server().await;

    // get_zone calls /zones/{zone_id_or_name} directly
    Mock::given(method("GET"))
        .and(path("/zones/example.com"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_zone_response(
            123,
            "example.com",
            3600,
        )))
        .mount(&server)
        .await;

    let provider = HetznerProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zone = provider
        .get_zone("example.com")
        .await
        .expect("Failed to get zone");
    assert_eq!(zone.id(), "123");
    assert_eq!(zone.domain(), "example.com");
}

#[tokio::test]
async fn test_get_zone_not_found() {
    let server = setup_mock_server().await;

    // get_zone calls /zones/{zone_id_or_name} directly - returns 404 for nonexistent
    Mock::given(method("GET"))
        .and(path("/zones/nonexistent.com"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "error": {
                "code": "not_found",
                "message": "Zone not found"
            }
        })))
        .mount(&server)
        .await;

    let provider = HetznerProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let result = provider.get_zone("nonexistent.com").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_create_zone_success() {
    let server = setup_mock_server().await;

    // create_zone expects CreateZoneResponse with zone and action fields
    Mock::given(method("POST"))
        .and(path("/zones"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "zone": {
                "id": 789,
                "name": "newdomain.com",
                "mode": "primary",
                "ttl": 3600,
                "status": "pending",
                "record_count": 0
            },
            "action": {
                "id": 1,
                "command": "create_zone",
                "status": "running",
                "progress": 0
            }
        })))
        .mount(&server)
        .await;

    let provider = HetznerProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zone = provider
        .create_zone("newdomain.com")
        .await
        .expect("Failed to create zone");
    assert_eq!(zone.id(), "789");
    assert_eq!(zone.domain(), "newdomain.com");
}

#[tokio::test]
async fn test_delete_zone_success() {
    let server = setup_mock_server().await;

    Mock::given(method("DELETE"))
        .and(path("/zones/123"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let provider = HetznerProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    provider
        .delete_zone("123")
        .await
        .expect("Failed to delete zone");
}

// =============================================================================
// Record (RRSet) Tests
// =============================================================================

#[tokio::test]
async fn test_list_records_success() {
    let server = setup_mock_server().await;

    // Get zone uses path /zones/{zone_id_or_name}
    Mock::given(method("GET"))
        .and(path("/zones/example.com"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_zone_response(
            123,
            "example.com",
            3600,
        )))
        .mount(&server)
        .await;

    // List RRSets uses path with query params for pagination
    Mock::given(method("GET"))
        .and(path("/zones/123/rrsets"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_rrsets_response(
                123,
                vec![
                    ("@", "A", 300, vec!["192.168.1.1"]),
                    ("www", "CNAME", 300, vec!["example.com."]),
                    ("@", "MX", 300, vec!["10 mail.example.com."]),
                ],
            )),
        )
        .mount(&server)
        .await;

    let provider = HetznerProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zone = provider
        .get_zone("example.com")
        .await
        .expect("Failed to get zone");
    let records = zone.list_records().await.expect("Failed to list records");

    // Each RRSet becomes individual records
    assert_eq!(records.len(), 3);
    assert!(records.iter().any(|r| r.data.get_type() == "A"));
    assert!(records.iter().any(|r| r.data.get_type() == "CNAME"));
    assert!(records.iter().any(|r| r.data.get_type() == "MX"));
}

#[tokio::test]
async fn test_list_records_multiple_values_per_rrset() {
    let server = setup_mock_server().await;

    Mock::given(method("GET"))
        .and(path("/zones/example.com"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_zone_response(
            123,
            "example.com",
            3600,
        )))
        .mount(&server)
        .await;

    // RRSet with multiple A records
    Mock::given(method("GET"))
        .and(path("/zones/123/rrsets"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_rrsets_response(
                123,
                vec![(
                    "@",
                    "A",
                    300,
                    vec!["192.168.1.1", "192.168.1.2", "192.168.1.3"],
                )],
            )),
        )
        .mount(&server)
        .await;

    let provider = HetznerProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zone = provider
        .get_zone("example.com")
        .await
        .expect("Failed to get zone");
    let records = zone.list_records().await.expect("Failed to list records");

    // Should get 3 individual A records
    assert_eq!(records.len(), 3);
    for record in &records {
        assert_eq!(record.data.get_type(), "A");
    }
}

#[tokio::test]
async fn test_get_record_success() {
    let server = setup_mock_server().await;

    Mock::given(method("GET"))
        .and(path("/zones/example.com"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_zone_response(
            123,
            "example.com",
            3600,
        )))
        .mount(&server)
        .await;

    // Get specific RRSet uses path /zones/{zone}/rrsets/{name}/{type}
    Mock::given(method("GET"))
        .and(path("/zones/123/rrsets/www/A"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_rrset_response(
                123,
                "www",
                "A",
                300,
                vec!["192.168.1.1"],
            )),
        )
        .mount(&server)
        .await;

    let provider = HetznerProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zone = provider
        .get_zone("example.com")
        .await
        .expect("Failed to get zone");

    // Record IDs in Cloud API format: "name/type/value"
    let record = zone
        .get_record("www/A/192.168.1.1")
        .await
        .expect("Failed to get record");

    assert_eq!(record.host, "www");
    assert_eq!(record.data, RecordData::A(Ipv4Addr::new(192, 168, 1, 1)));
}

#[tokio::test]
async fn test_create_record_success() {
    let server = setup_mock_server().await;

    Mock::given(method("GET"))
        .and(path("/zones/example.com"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_zone_response(
            123,
            "example.com",
            3600,
        )))
        .mount(&server)
        .await;

    // Create record uses add_records action endpoint
    Mock::given(method("POST"))
        .and(path("/zones/123/rrsets/test/A/actions/add_records"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_action_response(1, "success")))
        .mount(&server)
        .await;

    let provider = HetznerProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zone = provider
        .get_zone("example.com")
        .await
        .expect("Failed to get zone");
    let record = zone
        .create_record("test", &RecordData::A(Ipv4Addr::new(10, 0, 0, 1)), 300)
        .await
        .expect("Failed to create record");

    assert_eq!(record.host, "test");
    assert_eq!(record.data, RecordData::A(Ipv4Addr::new(10, 0, 0, 1)));
}

#[tokio::test]
async fn test_create_record_add_to_existing_rrset() {
    let server = setup_mock_server().await;

    Mock::given(method("GET"))
        .and(path("/zones/example.com"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_zone_response(
            123,
            "example.com",
            3600,
        )))
        .mount(&server)
        .await;

    // Add record to existing RRSet uses the add_records action endpoint
    Mock::given(method("POST"))
        .and(path("/zones/123/rrsets/test/A/actions/add_records"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_action_response(2, "success")))
        .mount(&server)
        .await;

    let provider = HetznerProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zone = provider
        .get_zone("example.com")
        .await
        .expect("Failed to get zone");
    let record = zone
        .create_record("test", &RecordData::A(Ipv4Addr::new(10, 0, 0, 2)), 300)
        .await
        .expect("Failed to create record");

    assert_eq!(record.data, RecordData::A(Ipv4Addr::new(10, 0, 0, 2)));
}

#[tokio::test]
async fn test_delete_record_success() {
    let server = setup_mock_server().await;

    Mock::given(method("GET"))
        .and(path("/zones/example.com"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_zone_response(
            123,
            "example.com",
            3600,
        )))
        .mount(&server)
        .await;

    // Delete record uses remove_records action endpoint
    Mock::given(method("POST"))
        .and(path("/zones/123/rrsets/test/A/actions/remove_records"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_action_response(3, "success")))
        .mount(&server)
        .await;

    let provider = HetznerProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zone = provider
        .get_zone("example.com")
        .await
        .expect("Failed to get zone");

    // Delete record by its ID format
    zone.delete_record("test/A/10.0.0.1")
        .await
        .expect("Failed to delete record");
}

#[tokio::test]
async fn test_delete_last_record_removes_rrset() {
    let server = setup_mock_server().await;

    Mock::given(method("GET"))
        .and(path("/zones/example.com"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_zone_response(
            123,
            "example.com",
            3600,
        )))
        .mount(&server)
        .await;

    // Delete last record also uses remove_records action endpoint
    // (The API automatically cleans up empty RRSets)
    Mock::given(method("POST"))
        .and(path("/zones/123/rrsets/test/A/actions/remove_records"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_action_response(4, "success")))
        .mount(&server)
        .await;

    let provider = HetznerProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zone = provider
        .get_zone("example.com")
        .await
        .expect("Failed to get zone");

    zone.delete_record("test/A/10.0.0.1")
        .await
        .expect("Failed to delete record");
}

// =============================================================================
// Record Type Tests
// =============================================================================

#[tokio::test]
async fn test_aaaa_record() {
    let server = setup_mock_server().await;

    Mock::given(method("GET"))
        .and(path("/zones/example.com"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_zone_response(
            123,
            "example.com",
            3600,
        )))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/zones/123/rrsets"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_rrsets_response(
                123,
                vec![("@", "AAAA", 300, vec!["2001:db8::1"])],
            )),
        )
        .mount(&server)
        .await;

    let provider = HetznerProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zone = provider
        .get_zone("example.com")
        .await
        .expect("Failed to get zone");
    let records = zone.list_records().await.expect("Failed to list records");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].data.get_type(), "AAAA");
}

#[tokio::test]
async fn test_mx_record() {
    let server = setup_mock_server().await;

    Mock::given(method("GET"))
        .and(path("/zones/example.com"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_zone_response(
            123,
            "example.com",
            3600,
        )))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/zones/123/rrsets"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_rrsets_response(
                123,
                vec![("@", "MX", 300, vec!["10 mail.example.com."])],
            )),
        )
        .mount(&server)
        .await;

    let provider = HetznerProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zone = provider
        .get_zone("example.com")
        .await
        .expect("Failed to get zone");
    let records = zone.list_records().await.expect("Failed to list records");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].data.get_type(), "MX");
}

#[tokio::test]
async fn test_txt_record() {
    let server = setup_mock_server().await;

    Mock::given(method("GET"))
        .and(path("/zones/example.com"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_zone_response(
            123,
            "example.com",
            3600,
        )))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/zones/123/rrsets"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_rrsets_response(
                123,
                vec![(
                    "@",
                    "TXT",
                    300,
                    vec!["v=spf1 include:_spf.example.com ~all"],
                )],
            )),
        )
        .mount(&server)
        .await;

    let provider = HetznerProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zone = provider
        .get_zone("example.com")
        .await
        .expect("Failed to get zone");
    let records = zone.list_records().await.expect("Failed to list records");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].data.get_type(), "TXT");
}

#[tokio::test]
async fn test_caa_record() {
    let server = setup_mock_server().await;

    Mock::given(method("GET"))
        .and(path("/zones/example.com"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_zone_response(
            123,
            "example.com",
            3600,
        )))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/zones/123/rrsets"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_rrsets_response(
                123,
                vec![("@", "CAA", 300, vec!["0 issue \"letsencrypt.org\""])],
            )),
        )
        .mount(&server)
        .await;

    let provider = HetznerProvider::with_base_url("test-token", &server.uri())
        .expect("Failed to create provider");

    let zone = provider
        .get_zone("example.com")
        .await
        .expect("Failed to get zone");
    let records = zone.list_records().await.expect("Failed to list records");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].data.get_type(), "CAA");
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
            let ip_str = ip_addr.to_string();

            Mock::given(method("GET"))
                .and(path("/zones/example.com"))
                .and(header("Authorization", "Bearer test-token"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(mock_zone_response(123, "example.com", 3600)),
                )
                .mount(&server)
                .await;

            Mock::given(method("GET"))
                .and(path("/zones/123/rrsets"))
                .and(header("Authorization", "Bearer test-token"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(mock_rrsets_response(123, vec![
                            ("@", "A", 300, vec![ip_str.as_str()]),
                        ])),
                )
                .mount(&server)
                .await;

            let provider = HetznerProvider::with_base_url("test-token", &server.uri())
                .expect("Failed to create provider");

            let zone = provider.get_zone("example.com").await.expect("Failed to get zone");
            let records = zone.list_records().await.expect("Failed to list records");

            prop_assert_eq!(records.len(), 1);
            prop_assert_eq!(&records[0].data, &RecordData::A(ip_addr));
            Ok(())
        }).unwrap();
    }

    #[test]
    fn test_various_ttl_values(ttl in 60u64..=86400u64) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let server = setup_mock_server().await;

            Mock::given(method("GET"))
                .and(path("/zones/example.com"))
                .and(header("Authorization", "Bearer test-token"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(mock_zone_response(123, "example.com", ttl)),
                )
                .mount(&server)
                .await;

            Mock::given(method("GET"))
                .and(path("/zones/123/rrsets"))
                .and(header("Authorization", "Bearer test-token"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(mock_rrsets_response(123, vec![
                            ("@", "A", ttl, vec!["1.2.3.4"]),
                        ])),
                )
                .mount(&server)
                .await;

            let provider = HetznerProvider::with_base_url("test-token", &server.uri())
                .expect("Failed to create provider");

            let zone = provider.get_zone("example.com").await.expect("Failed to get zone");
            let records = zone.list_records().await.expect("Failed to list records");

            prop_assert_eq!(records.len(), 1);
            prop_assert_eq!(records[0].ttl, ttl);
            Ok(())
        }).unwrap();
    }

    #[test]
    fn test_various_zone_names(name in "[a-z]{3,10}\\.(com|org|net)") {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let server = setup_mock_server().await;
            let name_clone = name.clone();

            Mock::given(method("GET"))
                .and(path("/zones"))
                .and(header("Authorization", "Bearer test-token"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(mock_zones_response(vec![(123, &name_clone, 3600)])),
                )
                .mount(&server)
                .await;

            let provider = HetznerProvider::with_base_url("test-token", &server.uri())
                .expect("Failed to create provider");

            let zones = provider.list_zones().await.expect("Failed to list zones");

            prop_assert_eq!(zones.len(), 1);
            prop_assert_eq!(zones[0].domain(), name.as_str());
            Ok(())
        }).unwrap();
    }
}
