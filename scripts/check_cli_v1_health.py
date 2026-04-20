#!/usr/bin/env python3
"""
Validate and enforce hard guards for CLI v1 health report.

Usage:
  python scripts/check_cli_v1_health.py
  python scripts/check_cli_v1_health.py --report artifacts/cli-v1-health.json
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_REPORT = ROOT / "artifacts/cli-v1-health.json"
DEFAULT_SCHEMA = ROOT / "schemas/cli/cli-v1-health.schema.json"


def _load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _validate_shape(report: dict, schema: dict) -> list[str]:
    bad: list[str] = []
    required = schema.get("required", [])
    props = schema.get("properties", {})
    if not isinstance(required, list) or not isinstance(props, dict):
        return ["invalid schema: top-level required/properties"]

    for key in required:
        if key not in report:
            bad.append(f"missing required top-level field `{key}`")
    for key in report:
        if key not in props:
            bad.append(f"unexpected top-level field `{key}`")
    return bad


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--report", default=str(DEFAULT_REPORT.relative_to(ROOT)))
    ap.add_argument("--schema", default=str(DEFAULT_SCHEMA.relative_to(ROOT)))
    ap.add_argument(
        "--require-metric-mode",
        choices=("proxy", "observed"),
        default=None,
        help="require summary.metric_mode to be the given value",
    )
    ap.add_argument(
        "--require-observed-source",
        choices=("ci_real_run", "smoke_fixture", "flat_jsonl"),
        default=None,
        help="require summary.observed_metrics_source to be the given value",
    )
    args = ap.parse_args()

    report_path = ROOT / args.report
    schema_path = ROOT / args.schema
    report = _load_json(report_path)
    schema = _load_json(schema_path)

    bad = _validate_shape(report, schema)
    summary = report.get("summary", {})
    if not isinstance(summary, dict):
        bad.append("`summary` must be an object")
        summary = {}

    if summary.get("hard_guard_passed") is not True:
        reasons = summary.get("blocking_reasons", [])
        if not isinstance(reasons, list):
            reasons = []
        joined = ", ".join(str(x) for x in reasons) if reasons else "unknown reason"
        bad.append(f"hard guard failed: {joined}")

    metric_mode = summary.get("metric_mode")
    if args.require_metric_mode is not None and metric_mode != args.require_metric_mode:
        bad.append(
            "metric_mode mismatch: "
            f"required `{args.require_metric_mode}`, got `{metric_mode}`"
        )

    if args.require_observed_source is not None:
        src = summary.get("observed_metrics_source")
        if src != args.require_observed_source:
            bad.append(
                "observed_metrics_source mismatch: "
                f"required `{args.require_observed_source}`, got `{src}`"
            )

    if bad:
        print("cli v1 health check failed:")
        for item in bad:
            print(f"  - {item}")
        return 1

    missing = summary.get("missing_required_metrics", [])
    if isinstance(missing, list) and missing:
        print("cli v1 health check: hard guards passed (DoD metrics pending)")
        print("missing required metrics: " + ", ".join(str(x) for x in missing))
    else:
        print("cli v1 health check: hard guards passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
