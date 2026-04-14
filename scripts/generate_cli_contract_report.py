#!/usr/bin/env python3
"""
Generate a machine-readable CLI contract change report for CI artifacts.

Usage:
  python scripts/generate_cli_contract_report.py
  python scripts/generate_cli_contract_report.py --base-ref origin/main --output artifacts/cli-contract-report.json
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

SCRIPTS_DIR = Path(__file__).resolve().parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import check_cli_contract_gate as gate
import cli_contract_lib as lib

METRICS_SCHEMA = "schemas/cli/metrics-event.json"
ERROR_KIND_MAP = "schemas/cli/error-kind-map.json"


def _cli_registry_changed(files: list[str]) -> bool:
    """True if any file under crates/envr-cli/src/cli/ (or legacy cli.rs) changed."""
    norm = [f.replace("\\", "/") for f in files]
    return any(
        f == "crates/envr-cli/src/cli.rs" or f.startswith("crates/envr-cli/src/cli/")
        for f in norm
    )
GOVERNANCE_INDEX = "schemas/cli/governance-index.json"


def error_kind_map_change_summary(old: dict, new: dict) -> dict:
    return gate.analyze_error_kind_map_change(old, new)["summary"]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--base-ref", default=None, help="base branch/ref (default: origin/main or GITHUB_BASE_REF)")
    ap.add_argument(
        "--output",
        default="artifacts/cli-contract-report.json",
        help="output json path (repo-relative)",
    )
    args = ap.parse_args()

    base_ref = gate.resolve_base_ref(args.base_ref)
    merge_base = lib.resolve_merge_base(base_ref)
    if not merge_base:
        print(f"unable to resolve merge-base with {base_ref}", file=sys.stderr)
        return 2

    files = lib.changed_files(merge_base)
    schema_changed = lib.changed_cli_schema_files(files)
    metrics_schema_changed = METRICS_SCHEMA in schema_changed
    error_kind_map_changed = ERROR_KIND_MAP in schema_changed
    governance_index_changed = GOVERNANCE_INDEX in schema_changed

    breaking_files: list[str] = []
    breaking_reasons: dict[str, str] = {}
    breaking_reasons_all: dict[str, list[str]] = {}
    unreadable_files: list[str] = []
    for path in schema_changed:
        old_state, old = gate.read_json_from_rev_state(merge_base, path)
        new_state, new = gate.read_json_head_state(path)
        if old_state == "invalid" or new_state == "invalid":
            unreadable_files.append(path)
            continue
        if old_state == "missing" and new_state == "missing":
            unreadable_files.append(path)
            continue
        if old_state == "missing" and new_state == "ok":
            continue
        if old_state == "ok" and new_state == "missing":
            breaking_files.append(path)
            breaking_reasons[path] = "schema file deleted from HEAD"
            breaking_reasons_all[path] = ["schema file deleted from HEAD"]
            continue
        if old is not None and new is not None:
            reason = gate.first_breaking_reason_for_path(path, old, new)
            if reason:
                breaking_files.append(path)
                breaking_reasons[path] = reason
                breaking_reasons_all[path] = gate.all_breaking_reasons_for_path(path, old, new)

    breaking_ids = sorted(gate.changed_schema_ids(breaking_files))
    migration_note_suggestion = ""
    if breaking_ids:
        joined = ", ".join(breaking_ids)
        migration_note_suggestion = (
            f"Migration note: breaking schema ids/codes changed: {joined}. "
            "Document consumer impact, fallback behavior, and upgrade steps."
        )

    metrics_schema_breaking = METRICS_SCHEMA in breaking_files
    metrics_schema_breaking_reasons_all: list[str] = []
    if metrics_schema_breaking and METRICS_SCHEMA in breaking_reasons_all:
        metrics_schema_breaking_reasons_all = breaking_reasons_all[METRICS_SCHEMA]

    bump = gate.recommend_bumps(schema_changed, breaking_files)
    error_kind_map_summary: dict | None = None
    error_kind_map_migration_note_hint: str | None = None
    governance_index_summary: dict | None = None
    governance_index_migration_note_hint: str | None = None
    if error_kind_map_changed:
        old = gate.read_json_from_rev(merge_base, ERROR_KIND_MAP) or {}
        new = gate.read_json_head(ERROR_KIND_MAP) or {}
        if isinstance(old, dict) and isinstance(new, dict):
            analysis = gate.analyze_error_kind_map_change(old, new)
            error_kind_map_summary = analysis["summary"]
            error_kind_map_migration_note_hint = analysis["migration_note_hint"]
    if governance_index_changed:
        old = gate.read_json_from_rev(merge_base, GOVERNANCE_INDEX) or {}
        new = gate.read_json_head(GOVERNANCE_INDEX) or {}
        if isinstance(old, dict) and isinstance(new, dict):
            analysis = gate.analyze_governance_index_change(old, new)
            governance_index_summary = analysis["summary"]
            governance_index_migration_note_hint = analysis["migration_note_hint"]

    report = {
        "base_ref": base_ref,
        "merge_base": merge_base,
        "changed_files": files,
        "schema_changed_files": schema_changed,
        "unreadable_schema_files": unreadable_files,
        "breaking_schema_files": breaking_files,
        "breaking_reasons": breaking_reasons,
        "breaking_reasons_all": breaking_reasons_all,
        "breaking_ids": breaking_ids,
        "migration_note_suggestion": migration_note_suggestion,
        "output_contract_changed": gate.OUTPUT_CONTRACT in files,
        "metrics_schema_changed": metrics_schema_changed,
        "metrics_schema_breaking": metrics_schema_breaking,
        "metrics_schema_breaking_reasons_all": metrics_schema_breaking_reasons_all,
        "error_kind_map_changed": error_kind_map_changed,
        "error_kind_map_change_summary": error_kind_map_summary,
        "error_kind_map_migration_note_hint": error_kind_map_migration_note_hint,
        "governance_index_changed": governance_index_changed,
        "governance_index_change_summary": governance_index_summary,
        "governance_index_migration_note_hint": governance_index_migration_note_hint,
        "capabilities_registry_changed": _cli_registry_changed(files),
        "recommended_bumps": bump["recommended_bumps"],
        "release_actions": bump["release_actions"],
        "report_version": 6,
    }

    out_path = Path(args.output)
    if not out_path.is_absolute():
        out_path = gate.ROOT / out_path
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

