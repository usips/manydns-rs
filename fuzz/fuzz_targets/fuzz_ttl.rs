#![no_main]

use libdns::types::Ttl;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: u32| {
    // Test Ttl::try_new with arbitrary values
    match Ttl::try_new(data) {
        Ok(ttl) => {
            // Valid TTL should be in valid range
            let val: u32 = ttl.into();
            assert!(val >= Ttl::MIN.into());
            assert!(val <= Ttl::MAX.into());
        }
        Err(_) => {
            // Invalid value - either too small or too large
            let min: u32 = Ttl::MIN.into();
            let max: u32 = Ttl::MAX.into();
            assert!(data < min || data > max);
        }
    }

    // Test Ttl::new which clamps
    let ttl = Ttl::new(data);
    let val: u32 = ttl.into();
    let min: u32 = Ttl::MIN.into();
    let max: u32 = Ttl::MAX.into();
    assert!(val >= min);
    assert!(val <= max);
});
