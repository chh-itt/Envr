#!/usr/bin/env python3
"""
Generate schemas/cli/index.json from CLI single sources of truth.

Sources:
  - success_codes: union of CommandSpec.success_messages in
      crates/envr-cli/src/cli/command_spec.rs
  - failure_codes: codes::err constants in
      crates/envr-cli/src/codes.rs

Usage:
  python scripts/generate_cli_schema_index.py
  python scripts/generate_cli_schema_index.py --check
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
COMMAND_SPEC_PATH = ROOT / "crates" / "envr-cli" / "src" / "cli" / "command_spec.rs"
CODES_PATH = ROOT / "crates" / "envr-cli" / "src" / "codes.rs"
INDEX_PATH = ROOT / "schemas" / "cli" / "index.json"

def parse_codes_registry(path: Path) -> tuple[list[str], list[str]]:
    ok: list[str] = []
    err: list[str] = []
    scope: str | None = None
    for raw in path.read_text(encoding="utf-8", errors="replace").splitlines():
        line = raw.strip()
        if line == "pub mod ok {":
            scope = "ok"
            continue
        if line == "pub mod err {":
            scope = "err"
            continue
        if line == "}":
            scope = None
            continue
        if not line.startswith("pub const ") or ': &str = "' not in line:
            continue
        try:
            value = line.split(': &str = "', 1)[1].split('"', 1)[0]
        except IndexError:
            continue
        if not value:
            continue
        if scope == "ok":
            ok.append(value)
        elif scope == "err":
            err.append(value)
    return sorted(set(ok)), sorted(set(err))


def parse_command_spec_success_messages(path: Path) -> list[str]:
    source = path.read_text(encoding="utf-8", errors="replace")
    messages: set[str] = set()
    marker = "CommandSpec::new("
    i = 0
    while True:
        start = source.find(marker, i)
        if start < 0:
            break
        depth = 0
        j = start
        while j < len(source):
            ch = source[j]
            if ch == "(":
                depth += 1
            elif ch == ")":
                depth -= 1
                if depth == 0:
                    break
            j += 1
        block = source[start : j + 1]
        pos = block.rfind("&[")
        if pos >= 0:
            end = block.find("]", pos)
            if end > pos:
                msg_list = block[pos + 2 : end]
                for token in msg_list.split(","):
                    token = token.strip()
                    if token.startswith('"') and token.endswith('"') and len(token) >= 2:
                        messages.add(token[1:-1])
        i = j + 1
    return sorted(messages)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--check",
        action="store_true",
        help="verify index is up to date; do not write",
    )
    args = ap.parse_args()

    success_codes = parse_command_spec_success_messages(COMMAND_SPEC_PATH)
    _, failure_codes = parse_codes_registry(CODES_PATH)
    generated = {
        "version": 1,
        "success_codes": success_codes,
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

