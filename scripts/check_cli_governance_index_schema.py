#!/usr/bin/env python3
"""Validate schemas/cli/governance-index.json against its schema contract."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
INDEX_PATH = ROOT / "schemas/cli/governance-index.json"
SCHEMA_PATH = ROOT / "schemas/cli/governance-index.schema.json"


def _load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _validate_instance_schema(instance: dict, schema: dict) -> list[str]:
    required = schema.get("required", [])
    properties = schema.get("properties", {})
    bad: list[str] = []
    if not isinstance(required, list) or not isinstance(properties, dict):
        return ["schemas/cli/governance-index.schema.json has invalid required/properties shape"]
    for key in required:
        if key not in instance:
            bad.append(f"schemas/cli/governance-index.json missing required field `{key}`")
    for key in instance:
        if key not in properties:
            bad.append(f"schemas/cli/governance-index.json has unexpected field `{key}`")
    return bad


def main() -> int:
    schema = _load(SCHEMA_PATH)
    instance = _load(INDEX_PATH)
    bad = _validate_instance_schema(instance, schema)
    if bad:
        print("governance index schema check failed:")
        for e in bad:
            print(f"  - {e}")
        return 1
    print("governance index schema check: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
