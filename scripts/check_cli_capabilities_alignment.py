#!/usr/bin/env python3
"""
P38.1: audit alignment between capabilities report and automation matrix.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_REPORT = ROOT / "artifacts/cli-capabilities-report.json"
GOVERNANCE_INDEX = ROOT / "schemas/cli/governance-index.json"


def load_report(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def load_governance_index(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


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
    row_map = governance.get("porcelain_matrix_rows", {})
    if not isinstance(row_map, dict):
        raise ValueError("schemas/cli/governance-index.json: porcelain_matrix_rows must be an object")

    bad: list[str] = []
    for cmd in report.get("commands", []):
        if cmd.get("contract_surface") != "both":
            continue
        trace = str(cmd.get("trace_name"))
        row = row_map.get(trace)
        if not isinstance(row, dict):
            bad.append(f"{trace}: missing governance mapping in porcelain_matrix_rows")
            continue
        expected = row.get("porcelain_expected")
        if expected is not True:
            bad.append(f"{trace}: governance index must set porcelain_expected=true")

    if bad:
        print("capabilities alignment check failed:")
        for line in bad:
            print(f"  - {line}")
        return 1
    print("capabilities alignment check: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

