#![no_main]

//! Coverage-guided fuzzing for the untrusted-source parser (#64).
//!
//! `parse_source` is the entry point for arbitrary Rust/Soroban source, so a
//! panic, unbounded recursion, or pathological slowdown here is a denial of
//! service. The target feeds arbitrary bytes decoded as lossy UTF-8 (matching
//! how the scanner reads non-UTF-8 input) and performs no filesystem access.

use libfuzzer_sys::fuzz_target;
use std::path::Path;

use soroban_scan_core::source::parse_source;

fuzz_target!(|data: &[u8]| {
    let content = String::from_utf8_lossy(data);
    let path = Path::new("fuzz/input.rs");
    let crate_root = Path::new("fuzz");
    let _ = parse_source(path, crate_root, &content);
});
