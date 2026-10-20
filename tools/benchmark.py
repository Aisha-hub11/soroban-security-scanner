#!/usr/bin/env python3
"""End-to-end CLI benchmark for `soroban-scan`.

The in-process `benchmark` example (see docs/development/benchmarks.md) measures
parsing and detection on synthetic sources. This script measures the *combined*
system end to end — project discovery, parsing, scanning and report generation —
by timing the real release binary over a target directory.

It is dependency-free (standard library only) and deterministic in what it
measures: the same target, the same number of runs, the same profile.

Usage:
    python tools/benchmark.py --target examples/vulnerable-contract --runs 10

The release binary is built automatically if it is missing.
"""

from __future__ import annotations

import argparse
import json
import statistics
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BIN = ROOT / "target" / "release" / (
    "soroban-scan.exe" if sys.platform.startswith("win") else "soroban-scan"
)
OUT = ROOT / "target" / "benchmark-report.json"
DEFAULT_BASELINE = ROOT / "tools" / "benchmark_baseline.json"
DEFAULT_THRESHOLD_FACTOR = 6.0


def summarize_samples(samples: list[float]) -> dict:
    """Return deterministic benchmark statistics for a list of millisecond samples."""
    ordered = sorted(samples)
    return {
        "runs": len(ordered),
        "min_ms": round(ordered[0], 3),
        "median_ms": round(statistics.median(ordered), 3),
        "mean_ms": round(statistics.mean(ordered), 3),
        "max_ms": round(ordered[-1], 3),
    }


def load_baseline(path: Path) -> dict:
    """Load and validate a benchmark baseline file."""
    with path.open("r", encoding="utf-8") as handle:
        data = json.load(handle)
    if data.get("schema_version") != 1:
        raise ValueError("unsupported benchmark baseline schema_version")
    if data.get("metric") != "median_ms":
        raise ValueError("only the median_ms benchmark metric is supported")
    baseline = data.get("baseline_median_ms")
    if not isinstance(baseline, (int, float)) or baseline <= 0:
        raise ValueError("baseline_median_ms must be a positive number")
    factor = data.get("threshold_factor", DEFAULT_THRESHOLD_FACTOR)
    if not isinstance(factor, (int, float)) or factor <= 1:
        raise ValueError("threshold_factor must be greater than 1")
    return data


def regression_errors(stats: dict, baseline: dict, factor: float | None = None) -> list[str]:
    """Return human-readable regression messages for a benchmark result.

    The comparison is intentionally one-sided: faster results never fail. A
    regression is reported only when the median exceeds
    ``baseline_median_ms * threshold_factor``.
    """
    allowed_factor = factor if factor is not None else baseline.get(
        "threshold_factor", DEFAULT_THRESHOLD_FACTOR
    )
    if allowed_factor <= 1:
        raise ValueError("threshold factor must be greater than 1")
    baseline_ms = float(baseline["baseline_median_ms"])
    limit_ms = baseline_ms * allowed_factor
    median_ms = float(stats["median_ms"])
    if median_ms <= limit_ms:
        return []
    return [
        (
            f"benchmark regression: median {median_ms:.1f} ms exceeds "
            f"{baseline_ms:.1f} ms baseline x {allowed_factor:g} "
            f"= {limit_ms:.1f} ms"
        )
    ]


def ensure_binary() -> None:
    if BIN.exists():
        return
    subprocess.run(
        ["cargo", "build", "--release", "-p", "soroban-scan-cli"],
        cwd=ROOT,
        check=True,
    )


def run_once(target: Path) -> float:
    start = time.perf_counter()
    subprocess.run(
        [
            str(BIN),
            "scan",
            str(target),
            "--format",
            "json",
            "--output",
            str(OUT),
            "--fail-on",
            "none",
            "--quiet",
        ],
        cwd=ROOT,
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return time.perf_counter() - start


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--target",
        default="examples/vulnerable-contract",
        help="directory to scan, relative to the repository root",
    )
    parser.add_argument(
        "--runs", type=int, default=10, help="measured runs after a warm-up"
    )
    parser.add_argument(
        "--baseline",
        type=Path,
        default=DEFAULT_BASELINE,
        help="recorded benchmark baseline JSON (default: tools/benchmark_baseline.json)",
    )
    parser.add_argument(
        "--threshold-factor",
        type=float,
        default=None,
        help="override the baseline threshold factor (must be > 1)",
    )
    parser.add_argument(
        "--json-out",
        type=Path,
        default=None,
        help="also write the measured statistics as JSON",
    )
    parser.add_argument(
        "--update-baseline",
        action="store_true",
        help="write the measured median into the baseline file (same runner only)",
    )
    args = parser.parse_args()

    target = (ROOT / args.target).resolve()
    if not target.exists():
        print(f"error: target not found: {target}", file=sys.stderr)
        return 2
    if args.runs < 1:
        print("error: --runs must be >= 1", file=sys.stderr)
        return 2
    if args.threshold_factor is not None and args.threshold_factor <= 1:
        print("error: --threshold-factor must be > 1", file=sys.stderr)
        return 2

    try:
        baseline = load_baseline(args.baseline)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"error: invalid benchmark baseline: {exc}", file=sys.stderr)
        return 2

    ensure_binary()
    run_once(target)  # warm-up
    samples = sorted(run_once(target) * 1000 for _ in range(args.runs))
    stats = summarize_samples(samples)

    print(f"target: {target.relative_to(ROOT)}")
    print(f"runs:   {stats['runs']}")
    print(f"min:    {stats['min_ms']:.1f} ms")
    print(f"median: {stats['median_ms']:.1f} ms")
    print(f"mean:   {stats['mean_ms']:.1f} ms")
    print(f"max:    {stats['max_ms']:.1f} ms")

    limit_ms = float(baseline["baseline_median_ms"]) * (
        args.threshold_factor
        if args.threshold_factor is not None
        else float(baseline.get("threshold_factor", DEFAULT_THRESHOLD_FACTOR))
    )
    print(
        f"baseline: {float(baseline['baseline_median_ms']):.1f} ms "
        f"(fail above {limit_ms:.1f} ms)"
    )

    if args.update_baseline:
        baseline["baseline_median_ms"] = stats["median_ms"]
        baseline["target"] = str(target.relative_to(ROOT)).replace("\\", "/")
        baseline["runner"] = "ubuntu-latest"
        if args.threshold_factor is not None:
            baseline["threshold_factor"] = args.threshold_factor
        args.baseline.write_text(
            json.dumps(baseline, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        print(f"updated baseline: {args.baseline}")
        return 0

    if args.json_out is not None:
        payload = dict(stats)
        payload["baseline_median_ms"] = baseline["baseline_median_ms"]
        payload["threshold_factor"] = baseline.get(
            "threshold_factor", DEFAULT_THRESHOLD_FACTOR
        )
        payload["limit_ms"] = limit_ms
        args.json_out.write_text(
            json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )

    errors = regression_errors(stats, baseline, args.threshold_factor)
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
