#!/usr/bin/env python3
"""
P38.2: audit offline-safe command coverage against Phase A coverage map.
"""

from __future__ import annotations

import argparse
import json
import re
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_REPORT = ROOT / "artifacts/cli-capabilities-report.json"
GOVERNANCE_INDEX = ROOT / "schemas/cli/governance-index.json"
DUE_DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")


def load_report(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def load_governance_index(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def collect_offline_alignment_failures(report: dict, governance: dict) -> list[str]:
    coverage_map = governance.get("offline_coverage_rows", {})
    if not isinstance(coverage_map, dict):
        raise ValueError("schemas/cli/governance-index.json: offline_coverage_rows must be an object")
    exempt_map = governance.get("offline_coverage_exempt", {})
    if not isinstance(exempt_map, dict):
        raise ValueError("schemas/cli/governance-index.json: offline_coverage_exempt must be an object")

    failures: list[str] = []
    expected_traces: set[str] = set()
    for cmd in report.get("commands", []):
        if not cmd.get("offline_safe", False):
            continue
        if cmd.get("contract_surface") not in {"json", "both"}:
            continue
        trace = str(cmd.get("trace_name"))
        if not trace:
            failures.append("report command has empty trace_name for offline-safe script-facing command")
            continue
        expected_traces.add(trace)
        row = coverage_map.get(trace)
        exemption = exempt_map.get(trace)
        valid_exemption = (
            isinstance(exemption, dict)
            and isinstance(exemption.get("reason"), str)
            and bool(exemption.get("reason").strip())
            and isinstance(exemption.get("owner"), str)
            and bool(exemption.get("owner").strip())
            and isinstance(exemption.get("exit_criteria"), str)
            and bool(exemption.get("exit_criteria").strip())
            and isinstance(exemption.get("due"), str)
            and bool(DUE_DATE_RE.fullmatch(exemption.get("due")))
        )
        if not isinstance(row, dict) and not valid_exemption:
            failures.append(f"{trace}: missing offline_coverage_rows mapping in governance-index")
            continue
        if isinstance(row, dict):
            if not isinstance(row.get("row_key"), str) or not row.get("row_key"):
                failures.append(f"{trace}: governance row_key must be a non-empty string")
            if row.get("network_skip_allowed") is not False:
                failures.append(f"{trace}: offline_safe command must set network_skip_allowed=false")
        if exemption is not None and not valid_exemption:
            failures.append(
                f"{trace}: offline_coverage_exempt must include non-empty reason/owner/exit_criteria and due=YYYY-MM-DD"
            )
        if valid_exemption:
            due = date.fromisoformat(exemption["due"])
            if due < date.today():
                failures.append(f"{trace}: offline_coverage_exempt is expired (due={exemption['due']})")

    for trace in sorted(coverage_map.keys()):
        if trace not in expected_traces:
            failures.append(
                f"{trace}: offline_coverage_rows contains stale mapping (not an offline-safe script-facing command)"
            )
    for trace in sorted(exempt_map.keys()):
        if trace not in expected_traces:
            failures.append(
                f"{trace}: offline_coverage_exempt contains stale mapping (not an offline-safe script-facing command)"
            )
    return failures


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--report",
        default=str(DEFAULT_REPORT),
        help="capabilities report path (default: artifacts/cli-capabilities-report.json)",
    )
    args = ap.parse_args()

    report_path = Path(args.report)
    if not report_path.is_absolute():
        report_path = ROOT / report_path
    report = load_report(report_path)
    governance = load_governance_index(GOVERNANCE_INDEX)
    failures = collect_offline_alignment_failures(report, governance)

    if failures:
        print("offline coverage alignment check failed:")
        for f in failures:
            print(f"  - {f}")
        return 1
    print("offline coverage alignment check: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

