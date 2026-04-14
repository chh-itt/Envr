#!/usr/bin/env python3
"""
Phase3: capability-driven test matrix audit.

Ensures governance-index capability_test_rows stays aligned with:
- capabilities report trace names
- docs/cli/automation-matrix.md Phase A coverage rows
"""

from __future__ import annotations

import argparse
import json
import re
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_REPORT = ROOT / "artifacts/cli-capabilities-report.json"
AUTOMATION_MATRIX = ROOT / "docs/cli/automation-matrix.md"
GOVERNANCE_INDEX = ROOT / "schemas/cli/governance-index.json"
DUE_DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")


def _load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def parse_phase_a_coverage(md: str) -> dict[str, dict[str, str]]:
    start = md.find("## Phase A coverage map")
    if start < 0:
        raise ValueError("phase A coverage map section not found")
    body = md[start:]
    out: dict[str, dict[str, str]] = {}
    for line in body.splitlines():
        row = line.strip()
        if not row.startswith("|"):
            continue
        if row.startswith("| Command / area ") or row.startswith("|----------------"):
            continue
        cols = [c.strip() for c in row.strip("|").split("|")]
        if len(cols) < 4:
            continue
        row_key = cols[0].replace("`", "").strip().lower()
        out[row_key] = {
            "json_ok": cols[1].strip(),
            "json_err": cols[2].strip(),
            "porcelain": cols[3].strip(),
        }
    return out


def _has_test_reference(cell: str) -> bool:
    s = cell.strip().lower()
    return s not in {"", "-", "—"}


def collect_capability_test_coverage_failures(
    report: dict, governance_index: dict, phase_rows: dict[str, dict[str, str]]
) -> list[str]:
    scope = governance_index.get("capability_test_rows", {})
    if not isinstance(scope, dict):
        raise ValueError("schemas/cli/governance-index.json: capability_test_rows must be an object")
    exempt = governance_index.get("capability_test_exempt", {})
    if not isinstance(exempt, dict):
        raise ValueError("schemas/cli/governance-index.json: capability_test_exempt must be an object")

    bad: list[str] = []
    trace_to_cmd: dict[str, dict] = {}
    expected_traces: set[str] = set()
    for cmd in report.get("commands", []):
        if cmd.get("contract_surface") not in {"json", "both"}:
            continue
        trace = str(cmd.get("trace_name"))
        if not trace:
            bad.append("capabilities report contains script-facing command with empty trace_name")
            continue
        expected_traces.add(trace)
        trace_to_cmd[trace] = cmd

    for trace, spec in scope.items():
        cmd = trace_to_cmd.get(trace)
        if cmd is None:
            bad.append(f"{trace}: capability_test_rows entry missing from report script-facing commands")
            continue
        if not isinstance(spec, dict):
            bad.append(f"{trace}: capability_test_rows entry must be object")
            continue
        row_key = str(spec.get("phase_a_row_key", "")).strip().lower()
        if not row_key:
            bad.append(f"{trace}: capability_test_rows.phase_a_row_key must be non-empty string")
            continue
        row = phase_rows.get(row_key)
        if row is None:
            bad.append(f"{trace}: Phase A row `{row_key}` not found in docs/cli/automation-matrix.md")
            continue
        if bool(spec.get("json_ok_required", False)) and not _has_test_reference(row["json_ok"]):
            bad.append(f"{trace}: Phase A row `{row_key}` missing JSON-ok test references")
        if bool(spec.get("porcelain_required", False)) and not _has_test_reference(row["porcelain"]):
            bad.append(f"{trace}: Phase A row `{row_key}` missing porcelain test references")

    for trace, spec in exempt.items():
        valid_exempt = (
            isinstance(spec, dict)
            and isinstance(spec.get("reason"), str)
            and bool(spec.get("reason").strip())
            and isinstance(spec.get("owner"), str)
            and bool(spec.get("owner").strip())
            and isinstance(spec.get("exit_criteria"), str)
            and bool(spec.get("exit_criteria").strip())
            and isinstance(spec.get("due"), str)
            and bool(DUE_DATE_RE.fullmatch(spec.get("due")))
        )
        if not valid_exempt:
            bad.append(
                f"{trace}: capability_test_exempt must include non-empty reason/owner/exit_criteria and due=YYYY-MM-DD"
            )
            continue
        due = date.fromisoformat(spec["due"])
        if due < date.today():
            bad.append(f"{trace}: capability_test_exempt is expired (due={spec['due']})")

    missing = sorted(expected_traces - set(scope.keys()) - set(exempt.keys()))
    for trace in missing:
        bad.append(f"{trace}: missing capability_test_rows entry in governance-index")
    stale_exempt = sorted(set(exempt.keys()) - expected_traces)
    for trace in stale_exempt:
        bad.append(f"{trace}: capability_test_exempt contains stale mapping (not a script-facing command)")
    return bad


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

    report = _load_json(report_path)
    index = _load_json(GOVERNANCE_INDEX)
    phase_rows = parse_phase_a_coverage(AUTOMATION_MATRIX.read_text(encoding="utf-8"))

    bad = collect_capability_test_coverage_failures(report, index, phase_rows)

    if bad:
        print("capability test coverage check failed:")
        for e in bad:
            print(f"  - {e}")
        return 1
    print("capability test coverage check: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
