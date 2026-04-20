#!/usr/bin/env python3
"""
Validate observed CLI metrics quality before using them as v1 health input.

Usage:
  python scripts/check_cli_observed_metrics.py --report artifacts/cli-observed-metrics.json
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def _load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _in_range(v: object, lo: float, hi: float) -> bool:
    return isinstance(v, (int, float)) and lo <= float(v) <= hi


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--report", default="artifacts/cli-observed-metrics.json")
    ap.add_argument(
        "--min-sample-size",
        type=int,
        default=20,
        help="minimum sample_size required for observed metrics to be trusted",
    )
    ap.add_argument(
        "--require-source",
        choices=("ci_real_run", "smoke_fixture", "flat_jsonl"),
        default=None,
        help="require observed_source to match (for gating on main)",
    )
    args = ap.parse_args()

    report_path = Path(args.report)
    if not report_path.is_absolute():
        report_path = ROOT / report_path
    data = _load_json(report_path)

    required = [
        "bootstrap_success_rate",
        "daily_run_success_rate",
        "doctor_fix_recovery_rate",
        "time_to_first_success_p95_ms",
        "offline_safe_latency_p95_ms",
        "extension_over_new_command_ratio",
        "sample_size",
    ]
    bad: list[str] = []
    for key in required:
        if key not in data:
            bad.append(f"missing field `{key}`")

    for key in (
        "bootstrap_success_rate",
        "daily_run_success_rate",
        "doctor_fix_recovery_rate",
        "extension_over_new_command_ratio",
    ):
        if key in data and not _in_range(data[key], 0.0, 1.0):
            bad.append(f"`{key}` must be in [0,1], got {data[key]!r}")

    for key in ("time_to_first_success_p95_ms", "offline_safe_latency_p95_ms"):
        v = data.get(key)
        if not isinstance(v, int) or v < 0:
            bad.append(f"`{key}` must be non-negative integer, got {v!r}")

    sample_size = data.get("sample_size")
    if not isinstance(sample_size, int) or sample_size < 0:
        bad.append(f"`sample_size` must be non-negative integer, got {sample_size!r}")
    elif sample_size < args.min_sample_size:
        bad.append(
            f"`sample_size` too small for observed mode ({sample_size} < {args.min_sample_size}); "
            "use proxy mode or provide larger observed sample"
        )

    if args.require_source is not None:
        src = data.get("observed_source")
        if src != args.require_source:
            bad.append(
                f"`observed_source` mismatch: required {args.require_source!r}, got {src!r}"
            )

    if bad:
        print("cli observed metrics check failed:")
        for item in bad:
            print(f"  - {item}")
        return 1

    print(
        "cli observed metrics check: ok "
        f"(sample_size={sample_size}, min_sample_size={args.min_sample_size})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
