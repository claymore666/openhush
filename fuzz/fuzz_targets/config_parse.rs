//! Fuzz target for TOML config parsing.
//!
//! Ensures that malformed TOML input doesn't cause panics.

#![no_main]

use libfuzzer_sys::fuzz_target;
use openhush::Config;

fuzz_target!(|data: &[u8]| {
    // Only process valid UTF-8 strings
    if let Ok(s) = std::str::from_utf8(data) {
        // Should not panic on any TOML input
        let _ = toml::from_str::<Config>(s);
    }
});
