#![no_main]

use manydns::RecordData;
use libfuzzer_sys::fuzz_target;

// Test various DNS record type strings and values
fuzz_target!(|data: (&str, &str)| {
    let (typ, value) = data;
    // Try to parse as various record types
    let _ = RecordData::from_raw(typ, value);
});
