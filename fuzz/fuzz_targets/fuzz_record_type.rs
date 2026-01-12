#![no_main]

use libdns::types::RecordType;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    // Test RecordType parsing with arbitrary strings
    if let Some(record_type) = RecordType::from_str(data) {
        // Verify roundtrip via string
        let s = record_type.to_string();
        let parsed = RecordType::from_str(&s).expect("Should roundtrip");
        assert_eq!(record_type, parsed);

        // Verify roundtrip via u16
        let code = record_type.as_u16();
        let from_code = RecordType::from_u16(code).expect("Should roundtrip from code");
        assert_eq!(record_type, from_code);
    }
});
