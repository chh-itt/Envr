#!/usr/bin/env python3
"""
Check sync between CommandSpec.help_path and help_registry/table.inc.

Rules:
1) Every CommandSpec.help_path must exist in HELP_BY_PATH.
2) Every HELP_BY_PATH entry must be either:
   - a CommandSpec.help_path, or
   - a strict prefix of at least one CommandSpec.help_path (group/container command).

Usage:
  python scripts/check_cli_help_registry_sync.py
  python scripts/check_cli_help_registry_sync.py --json
"""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
COMMAND_SPEC_PATH = ROOT / "crates" / "envr-cli" / "src" / "cli" / "command_spec.rs"
HELP_TABLE_PATH = ROOT / "crates" / "envr-cli" / "src" / "cli" / "help_registry" / "table.inc"


def parse_paths_from_help_table(path: Path) -> set[tuple[str, ...]]:
    src = path.read_text(encoding="utf-8", errors="replace")
    # match: path: &["foo", "bar"],
    path_re = re.compile(r'path:\s*&\[(?P<body>[^\]]*)\]')
    token_re = re.compile(r'"([^"]+)"')
    out: set[tuple[str, ...]] = set()
    for m in path_re.finditer(src):
        body = m.group("body")
        toks = tuple(token_re.findall(body))
        if toks:
            out.add(toks)
    return out


def parse_paths_from_command_spec(path: Path) -> set[tuple[str, ...]]:
    src = path.read_text(encoding="utf-8", errors="replace")
    # Capture the help_path list (second trailing &[...]) inside CommandSpec::new(...)
    row_re = re.compile(
        r"(?s)CommandSpec::new\(\s*\"[^\"]+\"\s*,.*?,\s*&\[(?P<help>[^\]]*)\]\s*,\s*&\[[^\]]*\]\s*\)"
    )
    token_re = re.compile(r'"([^"]+)"')
    out: set[tuple[str, ...]] = set()
    for row in row_re.finditer(src):
        help_body = row.group("help")
        toks = tuple(token_re.findall(help_body))
        if toks:
            out.add(toks)
    return out


def is_prefix_of_any(path: tuple[str, ...], all_paths: set[tuple[str, ...]]) -> bool:
    for other in all_paths:
        if len(other) <= len(path):
            continue
        if other[: len(path)] == path:
            return True
    return False


def fmt(path: tuple[str, ...]) -> str:
    return "/".join(path)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", action="store_true", help="print machine-readable report")
    args = ap.parse_args()

    spec_paths = parse_paths_from_command_spec(COMMAND_SPEC_PATH)
    help_paths = parse_paths_from_help_table(HELP_TABLE_PATH)

    missing_in_help = sorted(spec_paths - help_paths)
    orphan_in_help: list[tuple[str, ...]] = []
    for p in sorted(help_paths):
        if p in spec_paths:
            continue
        if is_prefix_of_any(p, spec_paths):
            continue
        orphan_in_help.append(p)

    ok = not missing_in_help and not orphan_in_help
    report = {
        "ok": ok,
        "command_spec_count": len(spec_paths),
        "help_registry_count": len(help_paths),
        "missing_in_help_registry": [fmt(p) for p in missing_in_help],
        "orphan_help_registry_paths": [fmt(p) for p in orphan_in_help],
        "suggestion": (
            "If command specs changed, update help_registry/table.inc. "
            "If help-only group paths are intentional, ensure they prefix at least one CommandSpec.help_path."
        ),
    }

    if args.json:
        print(json.dumps(report, ensure_ascii=False, indent=2))
    else:
        if ok:
            print(
                "help registry sync check: ok "
                f"(command_spec={len(spec_paths)}, help_registry={len(help_paths)})"
            )
        else:
            print("help registry sync check failed:")
            if missing_in_help:
                print("  missing_in_help_registry:")
                for p in missing_in_help:
                    print(f"    - {fmt(p)}")
            if orphan_in_help:
                print("  orphan_help_registry_paths:")
                for p in orphan_in_help:
                    print(f"    - {fmt(p)}")
            print("  suggestion:")
            print(f"    {report['suggestion']}")
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
