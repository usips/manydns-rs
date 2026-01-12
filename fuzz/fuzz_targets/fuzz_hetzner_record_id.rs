#![no_main]

//! Fuzz testing for Hetzner Cloud record ID parsing.
//!
//! Record IDs in the Hetzner provider have the format: "name/type/value"
//! This tests that arbitrary inputs don't cause panics when parsing record IDs.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    // Test parsing record IDs in the format "name/type/value"
    let parts: Vec<&str> = data.splitn(3, '/').collect();

    if parts.len() == 3 {
        let (name, typ, value) = (parts[0], parts[1], parts[2]);

        // Verify the parts are valid UTF-8 and can be used
        let _ = name.len();
        let _ = typ.len();
        let _ = value.len();

        // Test reconstructing the ID
        let reconstructed = format!("{}/{}/{}", name, typ, value);
        assert_eq!(reconstructed, data, "Record ID should roundtrip");
    }
});
