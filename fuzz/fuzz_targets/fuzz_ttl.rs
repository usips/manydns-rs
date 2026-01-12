#![no_main]

use libdns::types::Ttl;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: u32| {
    // Test Ttl::try_new with arbitrary values
    match Ttl::try_new(data) {
        Some(ttl) => {
            // Valid TTL should be in valid range
            let val = ttl.as_secs();
            assert!(val <= Ttl::MAX.as_secs());
        }
        None => {
            // Invalid value - too large
            assert!(data > Ttl::MAX.as_secs());
        }
    }

    // Test Ttl::new which clamps
    let ttl = Ttl::new(data);
    let val = ttl.as_secs();
    assert!(val <= Ttl::MAX.as_secs());
});
