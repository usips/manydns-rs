//! Unit tests for core DNS types.
//!
//! Tests Label, DomainName, Ttl, RecordType, and related type properties.

use libdns::types::{DomainName, Label, RecordClass, RecordType, Ttl, MAX_TTL};

#[test]
fn test_label_size() {
    // Label should be exactly 64 bytes (1 len + 63 data)
    assert_eq!(std::mem::size_of::<Label>(), 64);
}

#[test]
fn test_label_is_copy() {
    fn assert_copy<T: Copy>() {}
    assert_copy::<Label>();
}

#[test]
fn test_domain_name_size() {
    // DomainName should be exactly 256 bytes (1 len + 255 data)
    assert_eq!(std::mem::size_of::<DomainName>(), 256);
}

#[test]
fn test_ttl_size() {
    assert_eq!(std::mem::size_of::<Ttl>(), 4);
}

#[test]
fn test_ttl_is_copy() {
    fn assert_copy<T: Copy>() {}
    assert_copy::<Ttl>();
}

#[test]
fn test_label_creation() {
    let label = Label::from_str("example").unwrap();
    assert_eq!(label.len(), 7);
    assert_eq!(label.as_str(), Some("example"));

    // Too long
    let long = "a".repeat(64);
    assert!(Label::from_str(&long).is_none());

    // Max length is OK
    let max = "a".repeat(63);
    assert!(Label::from_str(&max).is_some());
}

#[test]
fn test_domain_name_creation() {
    let domain = DomainName::from_dotted("example.com").unwrap();
    assert_eq!(domain.to_dotted(), "example.com");

    let domain = DomainName::from_dotted("sub.example.com").unwrap();
    assert_eq!(domain.to_dotted(), "sub.example.com");

    // Root domain
    let root = DomainName::from_dotted("").unwrap();
    assert!(root.is_root());
}

#[test]
fn test_ttl_clamping() {
    let ttl = Ttl::new(u32::MAX);
    assert_eq!(ttl.as_secs(), MAX_TTL);

    let ttl = Ttl::new(3600);
    assert_eq!(ttl.as_secs(), 3600);
}

#[test]
fn test_record_type_roundtrip() {
    assert_eq!(RecordType::from_u16(1), Some(RecordType::A));
    assert_eq!(RecordType::A.as_u16(), 1);
    assert_eq!(RecordType::A.as_str(), "A");
}

#[test]
fn test_no_drop_for_copy_types() {
    fn assert_no_drop<T: Copy>() {
        // Copy types cannot have Drop
    }
    assert_no_drop::<Label>();
    assert_no_drop::<Ttl>();
    assert_no_drop::<RecordType>();
    assert_no_drop::<RecordClass>();
}
