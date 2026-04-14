#!/usr/bin/env python3
"""
Generate a reviewer-friendly markdown note from cli-contract-report artifact.

Usage:
  python scripts/generate_cli_contract_review_note.py
  python scripts/generate_cli_contract_review_note.py --report artifacts/cli-contract-report.json --output artifacts/cli-contract-review-note.md
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def _read_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _bool(v: bool) -> str:
    return "yes" if v else "no"


def render_review_note(report: dict) -> str:
    breaking_files = report.get("breaking_schema_files", [])
    release_actions = report.get("release_actions", [])
    migration_note = report.get("migration_note_suggestion", "")
    ek_changed = bool(report.get("error_kind_map_changed", False))
    ek_summary = report.get("error_kind_map_change_summary")
    ek_hint = report.get("error_kind_map_migration_note_hint")
    gi_changed = bool(report.get("governance_index_changed", False))
    gi_summary = report.get("governance_index_change_summary")
    gi_hint = report.get("governance_index_migration_note_hint")

    lines: list[str] = []
    lines.append("## CLI Contract Review Note")
    lines.append("")
    lines.append("### Snapshot")
    lines.append(
        f"- schema files changed: `{len(report.get('schema_changed_files', []))}`"
    )
    lines.append(f"- breaking schema detected: `{_bool(bool(breaking_files))}`")
    lines.append(f"- metrics schema changed: `{_bool(bool(report.get('metrics_schema_changed', False)))}`")
    lines.append(f"- error-kind-map changed: `{_bool(ek_changed)}`")
    lines.append(f"- governance-index changed: `{_bool(gi_changed)}`")
    lines.append("")

    if breaking_files:
        lines.append("### Breaking Files")
        for p in breaking_files:
            lines.append(f"- `{p}`")
        lines.append("")

    if release_actions:
        lines.append("### Release Actions")
        for a in release_actions:
            lines.append(f"- `{a}`")
        lines.append("")

    if migration_note:
        lines.append("### Migration Note Draft")
        lines.append(f"- {migration_note}")
        lines.append("")

    if ek_changed:
        lines.append("### Error Kind Map Change")
        if isinstance(ek_summary, dict):
            lines.append(f"- summary: `{json.dumps(ek_summary, ensure_ascii=False)}`")
        if isinstance(ek_hint, str) and ek_hint.strip():
            lines.append(f"- hint: {ek_hint}")
        lines.append("")

    if gi_changed:
        lines.append("### Governance Index Change")
        if isinstance(gi_summary, dict):
            lines.append(f"- summary: `{json.dumps(gi_summary, ensure_ascii=False)}`")
        if isinstance(gi_hint, str) and gi_hint.strip():
            lines.append(f"- hint: {gi_hint}")
        lines.append("")

    lines.append("### Suggested PR Comment")
    lines.append(
        "- Contract review completed. See `artifacts/cli-contract-report.json` and this note for breaking details and migration guidance."
    )
    return "\n".join(lines) + "\n"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--report",
        default="artifacts/cli-contract-report.json",
        help="input report path (repo-relative)",
    )
    ap.add_argument(
        "--output",
        default="artifacts/cli-contract-review-note.md",
        help="output markdown path (repo-relative)",
    )
    args = ap.parse_args()

    report_path = Path(args.report)
    if not report_path.is_absolute():
        report_path = ROOT / report_path
    out_path = Path(args.output)
    if not out_path.is_absolute():
        out_path = ROOT / out_path

    report = _read_json(report_path)
    note = render_review_note(report)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(note, encoding="utf-8")
    print(f"wrote {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
