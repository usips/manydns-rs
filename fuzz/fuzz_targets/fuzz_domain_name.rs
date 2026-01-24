#![no_main]

use manydns::types::DomainName;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    // Test DomainName creation with arbitrary strings
    if let Some(domain) = DomainName::from_dotted(data) {
        // Verify to_dotted produces a valid string
        let s = domain.to_dotted();
        // Re-parsing should succeed
        let reparsed = DomainName::from_dotted(&s);
        assert!(reparsed.is_some(), "Re-parsing to_dotted should succeed");
    }
});
