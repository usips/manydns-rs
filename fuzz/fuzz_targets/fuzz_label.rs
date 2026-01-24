#![no_main]

use manydns::types::Label;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Test Label creation with arbitrary bytes
    if let Some(label) = Label::new(data) {
        // Verify the bytes roundtrip
        let bytes = label.as_bytes();
        assert_eq!(bytes, data);

        // Length should match
        assert_eq!(label.len(), data.len());

        // Label shouldn't be marked as empty if data is non-empty
        assert_eq!(label.is_empty(), data.is_empty());
    } else {
        // Label creation fails for empty or too-long data
        assert!(data.is_empty() || data.len() > 63);
    }
});
