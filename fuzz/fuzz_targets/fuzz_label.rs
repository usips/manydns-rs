#![no_main]

use libdns::types::Label;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    // Test Label creation with arbitrary strings
    if let Ok(label) = Label::new(data) {
        // Verify roundtrip
        let s = label.as_ref();
        let _ = Label::new(s);

        // Check that to_bytes works
        let bytes = label.to_bytes();
        assert!(!bytes.is_empty());
        assert!(bytes.len() <= 64); // 1 length byte + max 63 char label
    }
});
