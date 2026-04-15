#!/usr/bin/env python3
"""
Generate machine-readable command capabilities report from cli registry.

Usage:
  python scripts/generate_cli_capabilities_report.py
  python scripts/generate_cli_capabilities_report.py --output artifacts/cli-capabilities-report.json
"""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
COMMAND_SPEC_RS = ROOT / "crates/envr-cli/src/cli/command_spec.rs"


def _parse_spec_rows(src: str) -> list[dict[str, object]]:
    rows: list[dict[str, object]] = []
    pattern = re.compile(
        r"\(CommandKey::(?P<key>\w+),\s*CommandSpec::new\("
        r"\"(?P<trace_name>[^\"]+)\",\s*"
        r"(?P<legacy_json_flag>None|Some\([^)]+\)),\s*"
        r"(?P<legacy_json_shorthand>true|false),\s*"
        r"(?P<runtime_required>true|false),\s*"
        r"(?P<runtime_group>None|Some\(RuntimeHandlerGroup::\w+\)),\s*"
        r"CommandCapabilities::new\("
        r"(?P<may_network>true|false),\s*"
        r"(?P<offline_safe>true|false),\s*"
        r"ContractSurface::(?P<contract_surface>\w+)\)",
        re.MULTILINE,
    )
    for m in pattern.finditer(src):
        runtime_group_raw = m.group("runtime_group")
        runtime_group: str | None
        if runtime_group_raw == "None":
            runtime_group = None
        else:
            runtime_group = runtime_group_raw.replace("Some(RuntimeHandlerGroup::", "").replace(")", "")
        rows.append(
            {
                "key": m.group("key"),
                "trace_name": m.group("trace_name"),
                "runtime_required": m.group("runtime_required") == "true",
                "runtime_group": runtime_group,
                "may_network": m.group("may_network") == "true",
                "offline_safe": m.group("offline_safe") == "true",
                "contract_surface": m.group("contract_surface").lower(),
            }
        )
    return rows


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--output",
        default="artifacts/cli-capabilities-report.json",
        help="output json path (repo-relative)",
    )
    args = ap.parse_args()

    src = COMMAND_SPEC_RS.read_text(encoding="utf-8")
    rows = _parse_spec_rows(src)
    rows = sorted(rows, key=lambda r: r["trace_name"])  # stable

    report = {"report_version": 1, "commands": rows}
    out = Path(args.output)
    if not out.is_absolute():
        out = ROOT / out
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

