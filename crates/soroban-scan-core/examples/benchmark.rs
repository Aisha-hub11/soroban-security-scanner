//! Lightweight in-process benchmark for detector throughput and peak memory.
//!
//! Run with:
//!
//! ```text
//! cargo run --release -p soroban-scan-core --example benchmark
//! ```
//!
//! This is intentionally dependency-free and is not part of the test gate. It
//! exists so contributors can measure the effect of detector changes.
//!
//! Peak resident memory is sampled from `/proc/self/status` (`VmHWM`) on Linux,
//! where CI runs this benchmark. On platforms without an equivalent zero-dependency
//! probe it prints `n/a` rather than guessing.

use std::time::Instant;

use soroban_scan_core::config::ScanConfig;
use soroban_scan_core::engine;
use soroban_scan_core::registry::RuleRegistry;

fn generate(entry_points: usize) -> String {
    let mut src = String::from("#[contractimpl]\nimpl Bench {\n");
    for i in 0..entry_points {
        src.push_str(&format!(
            "    pub fn f{i}(env: Env, who: Address, amount: i128) {{\n\
             \x20       who.require_auth();\n\
             \x20       let key = symbol_short!(\"K\");\n\
             \x20       env.storage().persistent().set(&key, &amount);\n\
             \x20       env.storage().persistent().extend_ttl(&key, 100, 1000);\n\
             \x20   }}\n"
        ));
    }
    src.push_str("}\n");
    src
}

/// Peak resident set size in bytes, or `None` when the platform cannot report it.
#[cfg(target_os = "linux")]
fn peak_rss_bytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmHWM:") {
            let kb: u64 = rest.split_whitespace().next()?.parse().ok()?;
            return Some(kb.saturating_mul(1024));
        }
    }
    None
}

#[cfg(not(target_os = "linux"))]
fn peak_rss_bytes() -> Option<u64> {
    None
}

fn format_bytes(bytes: Option<u64>) -> String {
    match bytes {
        Some(bytes) if bytes >= 1024 * 1024 => {
            format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
        }
        Some(bytes) => format!("{} KiB", bytes / 1024),
        None => "n/a".to_string(),
    }
}

fn main() {
    let registry = RuleRegistry::with_default_rules();
    let config = ScanConfig::default();

    println!(
        "{:>12} {:>10} {:>10} {:>10} {:>12} {:>10}",
        "entry_pts", "bytes", "findings", "files/s", "time", "peak_rss"
    );

    for entry_points in [100usize, 1_000, 5_000, 10_000] {
        let src = generate(entry_points);

        // Warm-up.
        let _ = engine::scan_source_str(&src, "bench.rs", &config, &registry);

        let start = Instant::now();
        let outcome = engine::scan_source_str(&src, "bench.rs", &config, &registry);
        let elapsed = start.elapsed();

        let per_second = entry_points as f64 / elapsed.as_secs_f64().max(f64::EPSILON);
        println!(
            "{:>12} {:>10} {:>10} {:>10.0} {:>12?} {:>10}",
            entry_points,
            src.len(),
            outcome.findings.len(),
            per_second,
            elapsed,
            format_bytes(peak_rss_bytes())
        );
    }
}
