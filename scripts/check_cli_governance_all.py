#!/usr/bin/env python3
"""
Run CLI governance checks through a single entrypoint.

Phase 1 rollout goal:
- provide one local command to run contract/sync checks
- reduce CI step sprawl for governance guardrails

Usage:
  python scripts/check_cli_governance_all.py
  python scripts/check_cli_governance_all.py --quick
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def run_step(label: str, command: list[str]) -> int:
    print(f"[governance] {label}")
    completed = subprocess.run(command, cwd=ROOT)
    if completed.returncode != 0:
        print(f"[governance] failed: {label}", file=sys.stderr)
        return completed.returncode
    print(f"[governance] ok: {label}")
    return 0


def build_steps(quick: bool) -> list[tuple[str, list[str]]]:
    steps: list[tuple[str, list[str]]] = [
        ("schema index up-to-date", ["python", "scripts/generate_cli_schema_index.py", "--check"]),
        ("dispatch/spec sync", ["python", "scripts/check_cli_dispatch_spec_sync.py"]),
        ("help registry sync", ["python", "scripts/check_cli_help_registry_sync.py"]),
        ("error kind map schema", ["python", "scripts/check_cli_error_kind_map_schema.py"]),
        ("governance index schema", ["python", "scripts/check_cli_governance_index_schema.py"]),
        ("governance exemptions", ["python", "scripts/check_cli_governance_exemptions.py"]),
        ("schema fragments integrity", ["python", "scripts/check_cli_schema_fragments.py"]),
        ("governance index sync", ["python", "scripts/check_cli_governance_index_sync.py"]),
    ]
    if not quick:
        steps.extend(
            [
                ("contract gate", ["python", "scripts/check_cli_contract_gate.py"]),
                ("contract gate unit tests", ["python", "scripts/test_cli_contract_gate.py"]),
            ]
        )
    return steps


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--quick",
        action="store_true",
        help="run only schema/sync checks (skip contract gate + gate tests)",
    )
    args = ap.parse_args()

    for label, cmd in build_steps(args.quick):
        code = run_step(label, cmd)
        if code != 0:
            return code

    print("[governance] all checks passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
