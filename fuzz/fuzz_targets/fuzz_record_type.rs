#![no_main]

use libdns::types::{RecordClass, RecordType};
use libfuzzer_sys::fuzz_target;
use std::str::FromStr;

fuzz_target!(|data: &str| {
    // Test RecordType parsing with arbitrary strings
    if let Ok(record_type) = RecordType::from_str(data) {
        // Verify roundtrip
        let s = record_type.to_string();
        let parsed = RecordType::from_str(&s).expect("Should roundtrip");
        assert_eq!(record_type, parsed);
    }

    // Test RecordClass parsing
    if let Ok(record_class) = RecordClass::from_str(data) {
        // Verify roundtrip
        let s = record_class.to_string();
        let parsed = RecordClass::from_str(&s).expect("Should roundtrip");
        assert_eq!(record_class, parsed);
    }
});
