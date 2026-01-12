#![no_main]

//! Fuzz testing for Hetzner TXT record value formatting.
//!
//! TXT records in Hetzner Cloud API must be wrapped in double quotes.
//! This tests that arbitrary TXT values are properly formatted.

use libfuzzer_sys::fuzz_target;

/// Format a TXT value for the Hetzner Cloud API.
/// TXT records must be wrapped in double quotes.
fn format_txt_value(val: &str) -> String {
    if val.starts_with('"') && val.ends_with('"') {
        val.to_string()
    } else {
        format!("\"{}\"", val)
    }
}

fuzz_target!(|data: &str| {
    let formatted = format_txt_value(data);

    // Verify the result always starts and ends with quotes
    assert!(formatted.starts_with('"'), "TXT value must start with quote");
    assert!(formatted.ends_with('"'), "TXT value must end with quote");

    // For non-empty input, the formatted string should be at least 2 chars (the quotes)
    assert!(formatted.len() >= 2, "TXT value must have at least quotes");

    // If input was already properly quoted, output should equal input
    if data.starts_with('"') && data.ends_with('"') && data.len() >= 2 {
        assert_eq!(formatted, data, "Already-quoted value should pass through");
    }
});
