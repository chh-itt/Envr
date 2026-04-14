#!/usr/bin/env python3
"""
Generate schemas/cli/index.json from envr-cli source literals.

Usage:
  python scripts/generate_cli_schema_index.py
  python scripts/generate_cli_schema_index.py --check
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CLI_SRC = ROOT / "crates" / "envr-cli" / "src"
INDEX_PATH = ROOT / "schemas" / "cli" / "index.json"

EMIT_OK_RE = re.compile(r'emit_ok\([^,]+,\s*"([a-zA-Z0-9_]+)"')
WRITE_ENVELOPE_OK_RE = re.compile(
    r'write_envelope\(\s*true\s*,\s*None\s*,\s*"([a-zA-Z0-9_]+)"'
)
EMIT_FAILURE_RE = re.compile(r'emit_failure_envelope\([^"]*"([a-zA-Z0-9_]+)"')


def collect_source_literals() -> tuple[list[str], list[str]]:
    data_messages: set[str] = set()
    failure_codes: set[str] = set()

    for path in sorted(CLI_SRC.rglob("*.rs")):
        src = path.read_text(encoding="utf-8", errors="replace")
        data_messages.update(m.group(1) for m in EMIT_OK_RE.finditer(src))
        data_messages.update(m.group(1) for m in WRITE_ENVELOPE_OK_RE.finditer(src))
        failure_codes.update(m.group(1) for m in EMIT_FAILURE_RE.finditer(src))

    return sorted(data_messages), sorted(failure_codes)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--check",
        action="store_true",
        help="verify index is up to date; do not write",
    )
    args = ap.parse_args()

    data_messages, failure_codes = collect_source_literals()
    generated = {
        "version": 1,
        "data_messages": data_messages,
        "failure_codes": failure_codes,
    }
    rendered = json.dumps(generated, indent=2) + "\n"

    if args.check:
        current = INDEX_PATH.read_text(encoding="utf-8")
        if current != rendered:
            print(
                "schemas/cli/index.json is out of date. "
                "Run: python scripts/generate_cli_schema_index.py",
                file=sys.stderr,
            )
            return 1
        print("ok: schemas/cli/index.json is up to date")
        return 0

    INDEX_PATH.write_text(rendered, encoding="utf-8")
    print(f"wrote {INDEX_PATH}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

