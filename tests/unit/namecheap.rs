//! Unit tests for Namecheap module helpers.
//!
//! Tests for domain splitting, record conversion, and XML parsing utilities.

use libdns::namecheap::{
    get_element_attr, host_record_to_record, parse_host_records, split_domain, ClientConfig,
    HostRecord,
};
use libdns::RecordData;

// =============================================================================
// Domain Splitting Tests
// =============================================================================

#[test]
fn test_split_domain() {
    assert_eq!(
        split_domain("example.com"),
        Some(("example".to_string(), "com".to_string()))
    );
    assert_eq!(
        split_domain("sub.example.com"),
        Some(("sub.example".to_string(), "com".to_string()))
    );
    assert_eq!(
        split_domain("example.co.uk"),
        Some(("example".to_string(), "co.uk".to_string()))
    );
    assert_eq!(
        split_domain("sub.example.co.uk"),
        Some(("sub.example".to_string(), "co.uk".to_string()))
    );
    assert_eq!(split_domain("com"), None);
}

// =============================================================================
// Host Record Conversion Tests
// =============================================================================

#[test]
fn test_host_record_to_record() {
    let hr = HostRecord {
        host_id: "123".to_string(),
        name: "www".to_string(),
        record_type: "A".to_string(),
        address: "1.2.3.4".to_string(),
        mx_pref: None,
        ttl: 3600,
    };

    let record = host_record_to_record(hr, "example.com");
    assert_eq!(record.id, "123");
    assert_eq!(record.host, "www.example.com");
    assert_eq!(record.data, RecordData::A("1.2.3.4".parse().unwrap()));
    assert_eq!(record.ttl, 3600);
}

#[test]
fn test_host_record_apex() {
    let hr = HostRecord {
        host_id: "456".to_string(),
        name: "@".to_string(),
        record_type: "A".to_string(),
        address: "1.2.3.4".to_string(),
        mx_pref: None,
        ttl: 1800,
    };

    let record = host_record_to_record(hr, "example.com");
    assert_eq!(record.host, "example.com");
}

#[test]
fn test_host_record_mx() {
    let hr = HostRecord {
        host_id: "789".to_string(),
        name: "@".to_string(),
        record_type: "MX".to_string(),
        address: "mail.example.com".to_string(),
        mx_pref: Some(10),
        ttl: 3600,
    };

    let record = host_record_to_record(hr, "example.com");
    assert_eq!(
        record.data,
        RecordData::MX {
            priority: 10,
            mail_server: "mail.example.com".to_string()
        }
    );
}

// =============================================================================
// XML Parsing Tests
// =============================================================================

#[test]
fn test_parse_host_records() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<ApiResponse xmlns="http://api.namecheap.com/xml.response" Status="OK">
  <Errors />
  <RequestedCommand>namecheap.domains.dns.getHosts</RequestedCommand>
  <CommandResponse Type="namecheap.domains.dns.getHosts">
    <DomainDNSGetHostsResult Domain="example.com" IsUsingOurDNS="true">
      <Host HostId="12" Name="@" Type="A" Address="1.2.3.4" MXPref="10" TTL="1800" />
      <Host HostId="14" Name="www" Type="A" Address="5.6.7.8" MXPref="10" TTL="1800" />
      <Host HostId="15" Name="mail" Type="MX" Address="mail.example.com" MXPref="10" TTL="3600" />
    </DomainDNSGetHostsResult>
  </CommandResponse>
</ApiResponse>"#;

    let records = parse_host_records(xml).unwrap();
    assert_eq!(records.len(), 3);

    assert_eq!(records[0].host_id, "12");
    assert_eq!(records[0].name, "@");
    assert_eq!(records[0].record_type, "A");
    assert_eq!(records[0].address, "1.2.3.4");
    assert_eq!(records[0].ttl, 1800);

    assert_eq!(records[1].name, "www");
    assert_eq!(records[2].record_type, "MX");
}

// =============================================================================
// Client Config Tests
// =============================================================================

#[test]
fn test_client_config_urls() {
    let sandbox = ClientConfig::sandbox("user", "key", "1.2.3.4");
    assert_eq!(sandbox.api_url(), "https://api.sandbox.namecheap.com/xml.response");
    assert!(sandbox.environment.is_sandbox());

    let prod = ClientConfig::production("user", "key", "1.2.3.4");
    assert_eq!(prod.api_url(), "https://api.namecheap.com/xml.response");
    assert!(prod.environment.is_production());
}

// =============================================================================
// XML Attribute Extraction Tests
// =============================================================================

#[test]
fn test_get_element_attr() {
    let xml = r#"<DomainDNSGetHostsResult Domain="example.com" IsUsingOurDNS="true">"#;
    assert_eq!(
        get_element_attr(xml, "DomainDNSGetHostsResult", "IsUsingOurDNS").unwrap(),
        Some("true".to_string())
    );
    assert_eq!(
        get_element_attr(xml, "DomainDNSGetHostsResult", "Domain").unwrap(),
        Some("example.com".to_string())
    );
    assert_eq!(
        get_element_attr(xml, "DomainDNSGetHostsResult", "NonExistent").unwrap(),
        None
    );
}
