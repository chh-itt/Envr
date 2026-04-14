#!/usr/bin/env python3
"""Validate schemas/cli/error-kind-map.json against its schema contract."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MAP_PATH = ROOT / "schemas/cli/error-kind-map.json"
SCHEMA_PATH = ROOT / "schemas/cli/error-kind-map.schema.json"


def _load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def main() -> int:
    schema = _load(SCHEMA_PATH)
    instance = _load(MAP_PATH)
    required = schema.get("required", [])
    properties = schema.get("properties", {})
    bad: list[str] = []
    if not isinstance(required, list) or not isinstance(properties, dict):
        bad.append("schemas/cli/error-kind-map.schema.json has invalid required/properties shape")
    else:
        for key in required:
            if key not in instance:
                bad.append(f"schemas/cli/error-kind-map.json missing required field `{key}`")
        for key in instance:
            if key not in properties:
                bad.append(f"schemas/cli/error-kind-map.json has unexpected field `{key}`")
    if bad:
        print("error kind map schema check failed:")
        for e in bad:
            print(f"  - {e}")
        return 1
    print("error kind map schema check: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
