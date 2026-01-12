//! Common test utilities shared across test modules.
//!
//! This module provides helpers for setting up mock servers, generating test data,
//! and other common testing patterns used throughout the test suite.

use wiremock::MockServer;

/// Sets up a new mock server for testing.
///
/// This is the standard way to create a mock server in tests.
pub async fn setup_mock_server() -> MockServer {
    MockServer::start().await
}

/// Test constants used across multiple test modules.
#[allow(dead_code)]
pub mod constants {
    /// Standard test token used in mock tests.
    pub const TEST_TOKEN: &str = "test-token";

    /// Standard bad token for unauthorized tests.
    pub const BAD_TOKEN: &str = "bad-token";
}

/// Cloudflare-specific mock helpers.
#[cfg(feature = "cloudflare")]
pub mod cloudflare {
    use serde_json::{json, Value};

    /// Cloudflare uses 32-char hex IDs.
    pub const ZONE_ID_1: &str = "aaaabbbbccccdddd1111222233334444";
    pub const ZONE_ID_2: &str = "eeeeffffaaaa00001111222233335555";
    pub const RECORD_ID_1: &str = "11112222333344445555666677778888";
    pub const RECORD_ID_2: &str = "88887777666655554444333322221111";
    pub const NEW_RECORD_ID: &str = "99990000aaaabbbbccccddddeeee0000";

    /// Creates a mock zone response.
    pub fn mock_zone_response(id: &str, name: &str) -> Value {
        json!({
            "success": true,
            "errors": [],
            "messages": [],
            "result": {
                "id": id,
                "name": name,
                "status": "active",
                "paused": false,
                "type": "full"
            }
        })
    }

    /// Creates a mock zones list response.
    pub fn mock_zones_list_response(zones: Vec<(&str, &str)>) -> Value {
        json!({
            "success": true,
            "errors": [],
            "messages": [],
            "result": zones.iter().map(|(id, name)| json!({
                "id": id,
                "name": name,
                "status": "active",
                "paused": false,
                "type": "full"
            })).collect::<Vec<_>>(),
            "result_info": {
                "page": 1,
                "per_page": 100,
                "total_pages": 1,
                "count": zones.len(),
                "total_count": zones.len()
            }
        })
    }

    /// Creates a mock record response.
    pub fn mock_record_response(
        id: &str,
        zone_id: &str,
        zone_name: &str,
        name: &str,
        record_type: &str,
        content: &str,
        ttl: u32,
    ) -> Value {
        json!({
            "success": true,
            "errors": [],
            "messages": [],
            "result": {
                "id": id,
                "zone_id": zone_id,
                "zone_name": zone_name,
                "name": name,
                "type": record_type,
                "content": content,
                "proxied": false,
                "ttl": ttl
            }
        })
    }

    /// Creates a mock records list response.
    pub fn mock_records_list_response(
        records: Vec<(&str, &str, &str, &str, &str, &str, u32)>,
    ) -> Value {
        json!({
            "success": true,
            "errors": [],
            "messages": [],
            "result": records.iter().map(|(id, zone_id, zone_name, name, record_type, content, ttl)| json!({
                "id": id,
                "zone_id": zone_id,
                "zone_name": zone_name,
                "name": name,
                "type": record_type,
                "content": content,
                "proxied": false,
                "ttl": ttl
            })).collect::<Vec<_>>(),
            "result_info": {
                "page": 1,
                "per_page": 100,
                "total_pages": 1,
                "count": records.len(),
                "total_count": records.len()
            }
        })
    }

    /// Creates a mock error response.
    pub fn mock_error_response(code: i32, message: &str) -> Value {
        json!({
            "success": false,
            "errors": [{"code": code, "message": message}],
            "messages": [],
            "result": null
        })
    }

    /// Creates a mock delete response.
    pub fn mock_delete_response(id: &str) -> Value {
        json!({
            "success": true,
            "errors": [],
            "messages": [],
            "result": {
                "id": id
            }
        })
    }
}

/// Hetzner-specific mock helpers.
#[cfg(feature = "hetzner")]
pub mod hetzner {
    use serde_json::{json, Value};

    /// Creates a mock zones list response (Cloud API format).
    pub fn mock_zones_response(zones: Vec<(u64, &str, u64)>) -> Value {
        json!({
            "meta": {
                "pagination": {
                    "page": 1,
                    "per_page": 25,
                    "previous_page": null,
                    "next_page": null,
                    "last_page": 1,
                    "total_entries": zones.len()
                }
            },
            "zones": zones.iter().map(|(id, name, ttl)| {
                json!({
                    "id": id,
                    "name": name,
                    "mode": "primary",
                    "ttl": ttl,
                    "status": "ok",
                    "record_count": 0
                })
            }).collect::<Vec<_>>()
        })
    }

    /// Creates a mock single zone response.
    pub fn mock_zone_response(id: u64, name: &str, ttl: u64) -> Value {
        json!({
            "zone": {
                "id": id,
                "name": name,
                "mode": "primary",
                "ttl": ttl,
                "status": "ok",
                "record_count": 0
            }
        })
    }

    /// Creates a mock RRSets list response with pagination.
    pub fn mock_rrsets_response(
        zone_id: u64,
        rrsets: Vec<(&str, &str, u64, Vec<&str>)>,
    ) -> Value {
        let len = rrsets.len();
        json!({
            "meta": {
                "pagination": {
                    "page": 1,
                    "per_page": 100,
                    "last_page": 1,
                    "total_entries": len
                }
            },
            "rrsets": rrsets.iter().map(|(name, record_type, ttl, values)| {
                json!({
                    "id": format!("{}/{}", name, record_type),
                    "name": name,
                    "type": record_type,
                    "ttl": ttl,
                    "zone": zone_id,
                    "records": values.iter().map(|v| json!({"value": v})).collect::<Vec<_>>()
                })
            }).collect::<Vec<_>>()
        })
    }

    /// Creates a mock single RRSet response.
    pub fn mock_rrset_response(
        zone_id: u64,
        name: &str,
        record_type: &str,
        ttl: u64,
        values: Vec<&str>,
    ) -> Value {
        json!({
            "rrset": {
                "id": format!("{}/{}", name, record_type),
                "name": name,
                "type": record_type,
                "ttl": ttl,
                "zone": zone_id,
                "records": values.iter().map(|v| json!({"value": v})).collect::<Vec<_>>()
            }
        })
    }

    /// Creates an action response for async operations.
    pub fn mock_action_response(id: u64, status: &str) -> Value {
        json!({
            "action": {
                "id": id,
                "command": "create_rrset",
                "status": status,
                "progress": 100
            }
        })
    }
}
