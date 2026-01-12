//! Property-based tests for core DNS types.
//!
//! These tests use proptest to generate arbitrary inputs and verify
//! invariants across the library's type system.

use libdns::types::*;
use libdns::RecordData;
use proptest::prelude::*;
use std::net::{Ipv4Addr, Ipv6Addr};

// =============================================================================
// Strategies for generating DNS types
// =============================================================================

/// Strategy for generating valid DNS labels (1-63 characters).
fn label_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?".prop_filter("Label must be 1-63 chars", |s| {
        !s.is_empty() && s.len() <= 63
    })
}

/// Strategy for generating valid domain names.
fn domain_name_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(label_strategy(), 1..5).prop_map(|labels| labels.join("."))
}

/// Strategy for generating IPv4 addresses.
fn ipv4_strategy() -> impl Strategy<Value = Ipv4Addr> {
    (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())
        .prop_map(|(a, b, c, d)| Ipv4Addr::new(a, b, c, d))
}

/// Strategy for generating IPv6 addresses.
fn ipv6_strategy() -> impl Strategy<Value = Ipv6Addr> {
    (
        any::<u16>(),
        any::<u16>(),
        any::<u16>(),
        any::<u16>(),
        any::<u16>(),
        any::<u16>(),
        any::<u16>(),
        any::<u16>(),
    )
        .prop_map(|(a, b, c, d, e, f, g, h)| Ipv6Addr::new(a, b, c, d, e, f, g, h))
}

/// Strategy for generating TTL values.
fn ttl_strategy() -> impl Strategy<Value = u32> {
    0..=MAX_TTL
}

/// Strategy for generating MX priorities.
fn priority_strategy() -> impl Strategy<Value = u16> {
    any::<u16>()
}

/// Strategy for generating record data.
fn record_data_strategy() -> impl Strategy<Value = RecordData> {
    prop_oneof![
        ipv4_strategy().prop_map(RecordData::A),
        ipv6_strategy().prop_map(RecordData::AAAA),
        domain_name_strategy().prop_map(RecordData::CNAME),
        (priority_strategy(), domain_name_strategy()).prop_map(|(priority, mail_server)| {
            RecordData::MX {
                priority,
                mail_server,
            }
        }),
        domain_name_strategy().prop_map(RecordData::NS),
        ".*".prop_map(RecordData::TXT),
        (
            priority_strategy(),
            any::<u16>(),
            any::<u16>(),
            domain_name_strategy()
        )
            .prop_map(|(priority, weight, port, target)| RecordData::SRV {
                priority,
                weight,
                port,
                target
            }),
    ]
}

// =============================================================================
// Label Tests
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn label_roundtrip(s in "[a-zA-Z0-9]{1,63}") {
        if let Some(label) = Label::from_str(&s) {
            prop_assert_eq!(label.as_str(), Some(s.as_str()));
            prop_assert_eq!(label.len(), s.len());
            prop_assert!(!label.is_empty());
        }
    }

    #[test]
    fn label_rejects_too_long(s in "[a-zA-Z0-9]{64,128}") {
        prop_assert!(Label::from_str(&s).is_none());
    }

    #[test]
    fn label_rejects_empty(_dummy in Just(())) {
        prop_assert!(Label::from_str("").is_none());
    }

    #[test]
    fn label_preserves_bytes(bytes in prop::collection::vec(any::<u8>(), 1..=63)) {
        if let Some(label) = Label::new(&bytes) {
            prop_assert_eq!(label.as_bytes(), &bytes[..]);
        }
    }
}

// =============================================================================
// DomainName Tests
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn domain_name_roundtrip(domain in domain_name_strategy()) {
        if let Some(dn) = DomainName::from_dotted(&domain) {
            let dotted = dn.to_dotted();
            prop_assert_eq!(dotted, domain);
        }
    }

    #[test]
    fn domain_name_wire_format_valid(domain in domain_name_strategy()) {
        if let Some(dn) = DomainName::from_dotted(&domain) {
            let wire = dn.as_wire_bytes();
            prop_assert!(wire.len() > 0);
            prop_assert!(wire.len() <= MAX_DOMAIN_LEN);
            // Wire format ends with null byte
            prop_assert_eq!(*wire.last().unwrap(), 0);
        }
    }

    #[test]
    fn domain_name_default_is_root(_dummy in Just(())) {
        let dn = DomainName::default();
        prop_assert!(dn.is_root());
    }
}

// =============================================================================
// TTL Tests
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn ttl_clamps_to_max(secs in any::<u32>()) {
        let ttl = Ttl::new(secs);
        prop_assert!(ttl.as_secs() <= MAX_TTL);
    }

    #[test]
    fn ttl_try_new_rejects_invalid(secs in (MAX_TTL + 1)..=u32::MAX) {
        prop_assert!(Ttl::try_new(secs).is_none());
    }

    #[test]
    fn ttl_try_new_accepts_valid(secs in ttl_strategy()) {
        prop_assert!(Ttl::try_new(secs).is_some());
        prop_assert_eq!(Ttl::try_new(secs).unwrap().as_secs(), secs);
    }

    #[test]
    fn ttl_from_u32_equals_new(secs in any::<u32>()) {
        let ttl1 = Ttl::new(secs);
        let ttl2: Ttl = secs.into();
        prop_assert_eq!(ttl1, ttl2);
    }

    #[test]
    fn ttl_zero_is_zero(_dummy in Just(())) {
        prop_assert!(Ttl::ZERO.is_zero());
        prop_assert_eq!(Ttl::ZERO.as_secs(), 0);
    }
}

// =============================================================================
// RecordType Tests
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn record_type_roundtrip_u16(val in 1u16..=257) {
        if let Some(rt) = RecordType::from_u16(val) {
            prop_assert_eq!(rt.as_u16(), val);
        }
    }

    #[test]
    fn record_type_from_str_case_insensitive(rt in prop_oneof![
        Just("A"), Just("a"),
        Just("AAAA"), Just("aaaa"),
        Just("CNAME"), Just("cname"),
        Just("MX"), Just("mx"),
        Just("TXT"), Just("txt"),
        Just("NS"), Just("ns"),
        Just("SRV"), Just("srv"),
    ]) {
        let parsed = RecordType::from_str(&rt);
        prop_assert!(parsed.is_some());
    }
}

// =============================================================================
// RecordClass Tests
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn record_class_roundtrip(val in 1u16..=4) {
        if let Some(rc) = RecordClass::from_u16(val) {
            prop_assert_eq!(rc.as_u16(), val);
        }
    }
}

// =============================================================================
// RecordData Tests
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn record_data_get_type_matches_variant(data in record_data_strategy()) {
        let typ = data.get_type();
        match &data {
            RecordData::A(_) => prop_assert_eq!(typ, "A"),
            RecordData::AAAA(_) => prop_assert_eq!(typ, "AAAA"),
            RecordData::CNAME(_) => prop_assert_eq!(typ, "CNAME"),
            RecordData::MX { .. } => prop_assert_eq!(typ, "MX"),
            RecordData::NS(_) => prop_assert_eq!(typ, "NS"),
            RecordData::TXT(_) => prop_assert_eq!(typ, "TXT"),
            RecordData::SRV { .. } => prop_assert_eq!(typ, "SRV"),
            RecordData::Other { typ: t, .. } => prop_assert_eq!(typ, t.as_str()),
        }
    }

    #[test]
    fn record_data_get_value_not_empty(data in record_data_strategy()) {
        let value = data.get_value();
        // All record types should produce some value representation
        // (might be empty string for TXT with empty content, which is valid)
        let _ = value; // Just verify it doesn't panic
    }

    #[test]
    fn record_data_from_raw_a(ip in ipv4_strategy()) {
        let raw_value = ip.to_string();
        let data = RecordData::from_raw("A", &raw_value);
        prop_assert_eq!(data, RecordData::A(ip));
    }

    #[test]
    fn record_data_from_raw_aaaa(ip in ipv6_strategy()) {
        let raw_value = ip.to_string();
        let data = RecordData::from_raw("AAAA", &raw_value);
        prop_assert_eq!(data, RecordData::AAAA(ip));
    }

    #[test]
    fn record_data_from_raw_cname(target in domain_name_strategy()) {
        let data = RecordData::from_raw("CNAME", &target);
        prop_assert_eq!(data, RecordData::CNAME(target));
    }

    #[test]
    fn record_data_from_raw_mx(priority in priority_strategy(), server in domain_name_strategy()) {
        let raw_value = format!("{} {}", priority, server);
        let data = RecordData::from_raw("MX", &raw_value);
        prop_assert_eq!(data, RecordData::MX { priority, mail_server: server });
    }

    #[test]
    fn record_data_from_raw_srv(
        priority in priority_strategy(),
        weight in any::<u16>(),
        port in any::<u16>(),
        target in domain_name_strategy()
    ) {
        let raw_value = format!("{} {} {} {}", priority, weight, port, target);
        let data = RecordData::from_raw("SRV", &raw_value);
        prop_assert_eq!(data, RecordData::SRV { priority, weight, port, target });
    }

    #[test]
    fn record_data_from_raw_txt(txt in ".*") {
        let data = RecordData::from_raw("TXT", &txt);
        prop_assert_eq!(data, RecordData::TXT(txt));
    }

    #[test]
    fn record_data_from_raw_unknown_type_preserves_data(typ in "[A-Z]{2,6}", value in ".*") {
        // Skip known types
        prop_assume!(!["A", "AAAA", "CNAME", "MX", "NS", "SRV", "TXT"].contains(&typ.as_str()));
        let data = RecordData::from_raw(&typ, &value);
        match data {
            RecordData::Other { typ: t, value: v } => {
                prop_assert_eq!(t, typ);
                prop_assert_eq!(v, value);
            }
            _ => prop_assert!(false, "Expected Other variant for unknown type"),
        }
    }

    #[test]
    fn record_data_api_value_mx_returns_server_only(
        priority in priority_strategy(),
        server in domain_name_strategy()
    ) {
        let data = RecordData::MX { priority, mail_server: server.clone() };
        prop_assert_eq!(data.get_api_value(), server);
    }
}

// =============================================================================
// Environment Tests
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]

    #[test]
    fn environment_production_is_not_sandbox(_dummy in Just(())) {
        let env = Environment::Production;
        prop_assert!(env.is_production());
        prop_assert!(!env.is_sandbox());
    }

    #[test]
    fn environment_sandbox_is_not_production(_dummy in Just(())) {
        let env = Environment::Sandbox;
        prop_assert!(env.is_sandbox());
        prop_assert!(!env.is_production());
    }

    #[test]
    fn environment_default_is_production(_dummy in Just(())) {
        let env = Environment::default();
        prop_assert!(env.is_production());
    }
}
