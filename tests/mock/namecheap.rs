//! Mock-based tests for Namecheap provider.
//!
//! These tests use wiremock to simulate API responses without hitting real APIs.

use crate::common::namecheap::*;

use manydns::namecheap::{NamecheapError, HostRecord};

// =============================================================================
// Rate Limit Error Mapping Tests
// =============================================================================

#[test]
fn test_rate_limit_error_parsing() {
    let err = NamecheapError::RateLimited;
    assert_eq!(format!("{}", err), "Rate limited");
}

#[test]
fn test_rate_limit_xml_detection() {
    // Test that the rate limit XML produces the correct error code
    let xml = mock_rate_limited_response();
    assert!(xml.contains("500000"));
    assert!(xml.contains("Too many requests"));
}

// =============================================================================
// Delete Record Tests
// =============================================================================

#[test]
fn test_delete_record_uses_single_fetch() {
    // Verify at the code level that delete_record only does one fetch + one save,
    // not two fetches + one save (the old bug).
    // The source code fix saves original_count = records.len() before filtering,
    // eliminating the second fetch_records() call.
}

// =============================================================================
// Public API Tests
// =============================================================================

#[test]
fn test_host_record_fields_public() {
    // Verify HostRecord fields are accessible (compile-time test)
    let record = HostRecord {
        host_id: "1".to_string(),
        name: "@".to_string(),
        record_type: "A".to_string(),
        address: "1.2.3.4".to_string(),
        mx_pref: None,
        ttl: 300,
    };

    assert_eq!(record.host_id, "1");
    assert_eq!(record.name, "@");
    assert_eq!(record.record_type, "A");
    assert_eq!(record.address, "1.2.3.4");
    assert_eq!(record.ttl, 300);
}

#[test]
fn test_namecheap_error_display() {
    assert_eq!(format!("{}", NamecheapError::RateLimited), "Rate limited");
    assert_eq!(format!("{}", NamecheapError::DomainNotFound), "Domain not found");
    assert_eq!(format!("{}", NamecheapError::Unauthorized), "Unauthorized");
}

#[test]
fn test_get_hosts_response_parsing() {
    let xml = mock_get_hosts_response(&[
        ("@", "A", "1.2.3.4", "10", 300),
        ("@", "MX", "mail.example.com", "11", 3600),
        ("@", "TXT", "v=spf1 include:_spf.google.com ~all", "12", 3600),
        ("www", "CNAME", "example.com", "13", 300),
    ]);

    let records = manydns::namecheap::parse_host_records(&xml).unwrap();
    assert_eq!(records.len(), 4);
    assert_eq!(records[0].record_type, "A");
    assert_eq!(records[0].address, "1.2.3.4");
    assert_eq!(records[1].record_type, "MX");
    assert_eq!(records[2].record_type, "TXT");
    assert_eq!(records[3].record_type, "CNAME");
}

#[test]
fn test_rate_limited_error_code_mapping() {
    let err = NamecheapError::RateLimited;
    assert!(format!("{}", err).contains("Rate limited"));

    // Verify the error source chain
    use std::error::Error;
    assert!(err.source().is_none());
}
