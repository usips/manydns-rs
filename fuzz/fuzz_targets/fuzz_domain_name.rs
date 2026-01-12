#![no_main]

use libdns::types::DomainName;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    // Test DomainName creation with arbitrary strings
    if let Ok(domain) = DomainName::new(data) {
        // Verify roundtrip
        let s: &str = domain.as_ref();
        let _ = DomainName::new(s);

        // Check wire format
        let wire = domain.to_wire_format();
        assert!(!wire.is_empty());
        // Wire format must end with null byte (root label)
        assert_eq!(wire.last(), Some(&0));

        // Check that labels work
        let labels = domain.labels();
        assert!(!labels.is_empty());
    }
});
