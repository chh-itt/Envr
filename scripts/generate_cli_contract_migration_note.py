#!/usr/bin/env python3
"""
P17 helper: draft migration notes for CLI schema/index changes.

Usage:
  python scripts/generate_cli_contract_migration_note.py
  python scripts/generate_cli_contract_migration_note.py --base-ref origin/main
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS_DIR = Path(__file__).resolve().parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import cli_contract_lib as lib
import check_cli_contract_gate as gate

ERROR_KIND_MAP_PATH = "schemas/cli/error-kind-map.json"
GOVERNANCE_INDEX_PATH = "schemas/cli/governance-index.json"


def run_git(args: list[str]) -> str:
    return lib.run_git(args)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--base-ref", default=None, help="base branch/ref (default: origin/main or GITHUB_BASE_REF)")
    args = ap.parse_args()

    base_ref = gate.resolve_base_ref(args.base_ref)
    merge_base = lib.resolve_merge_base(base_ref)
    if not merge_base:
        print(f"- Migration note: unable to resolve base ref ({base_ref}).")
        return 0

    files = lib.changed_files(merge_base)
    schema_changed = lib.changed_cli_schema_files(files)
    if not schema_changed:
        print("- Migration note: no CLI schema changes in this branch.")
        return 0

    breaking_files: list[str] = []
    non_breaking_files: list[str] = []
    for path in schema_changed:
        old = gate.read_json_from_rev(merge_base, path)
        new = gate.read_json_head(path)
        if old is None or new is None:
            non_breaking_files.append(path)
            continue
        if path == ERROR_KIND_MAP_PATH:
            breaking = gate.analyze_error_kind_map_change(old, new).get("breaking", False)
        elif path == GOVERNANCE_INDEX_PATH:
            breaking = gate.analyze_governance_index_change(old, new).get("breaking", False)
        else:
            breaking = gate.is_breaking_schema_change(old, new)
        if breaking:
            breaking_files.append(path)
        else:
            non_breaking_files.append(path)

    print("## Migration note draft")
    print()
    if breaking_files:
        print("- Migration note: breaking JSON contract change detected in:")
        for p in breaking_files:
            print(f"  - `{p}`")
        print("- Migration note: bump compatible schema/documentation version and provide consumer fallback guidance.")
    if non_breaking_files:
        print("- Migration note: non-breaking schema/index changes detected in:")
        for p in non_breaking_files:
            print(f"  - `{p}`")
        print("- Migration note: additive/compat updates only; existing parsers should continue working.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

