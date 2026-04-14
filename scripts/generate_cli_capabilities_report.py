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
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CLI_METADATA_RS = ROOT / "crates/envr-cli/src/cli/metadata.rs"


def _extract_paren_body(text: str, start: int) -> tuple[str, int]:
    depth = 0
    i = start
    while i < len(text):
        ch = text[i]
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
            if depth == 0:
                return text[start + 1 : i], i
        i += 1
    raise ValueError("unbalanced parentheses")


def _parse_metadata_rows(src: str) -> list[dict[str, object]]:
    rows: list[dict[str, object]] = []
    idx = 0
    while True:
        k = src.find("CommandKey::", idx)
        if k < 0:
            break
        key_start = k + len("CommandKey::")
        key_end = key_start
        while key_end < len(src) and (src[key_end].isalnum() or src[key_end] == "_"):
            key_end += 1
        key = src[key_start:key_end]

        md = src.find("CommandMetadata::new(", key_end)
        if md < 0:
            break
        md_args, md_end = _extract_paren_body(src, src.find("(", md))

        caps = md_args.find("CommandCapabilities::new(")
        if caps < 0:
            idx = md_end + 1
            continue
        cap_args, _ = _extract_paren_body(md_args, md_args.find("(", caps))

        # first fields before capabilities call
        before_caps = md_args[:caps].strip().rstrip(",")
        parts = [p.strip() for p in before_caps.split(",")]
        if len(parts) < 4:
            idx = md_end + 1
            continue
        trace_name = parts[0].strip().strip('"')
        runtime_required = parts[2] == "true"
        runtime_group = parts[3].replace("Some(RuntimeHandlerGroup::", "").replace(")", "")
        if parts[3] == "None":
            runtime_group = None

        cap_parts = [p.strip() for p in cap_args.split(",")]
        may_network = cap_parts[0] == "true"
        offline_safe = cap_parts[1] == "true"
        contract_surface = cap_parts[2].replace("ContractSurface::", "")

        rows.append(
            {
                "key": key,
                "trace_name": trace_name,
                "runtime_required": runtime_required,
                "runtime_group": runtime_group,
                "may_network": may_network,
                "offline_safe": offline_safe,
                "contract_surface": contract_surface.lower(),
            }
        )
        idx = md_end + 1
    return rows


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--output",
        default="artifacts/cli-capabilities-report.json",
        help="output json path (repo-relative)",
    )
    args = ap.parse_args()

    src = CLI_METADATA_RS.read_text(encoding="utf-8")
    rows = _parse_metadata_rows(src)
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

