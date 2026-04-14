#!/usr/bin/env python3
"""P37/P40: verify shared schema fragment consistency for CLI failure schemas."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FRAGMENT = ROOT / "schemas/cli/fragments/error_object.json"
ERROR_KIND_MAP = ROOT / "schemas/cli/error-kind-map.json"
ERROR_KIND_MAP_SCHEMA = ROOT / "schemas/cli/error-kind-map.schema.json"
OUTPUT_CONTRACT = ROOT / "docs/cli/output-contract.md"
OUTPUT_RS = ROOT / "crates/envr-cli/src/output.rs"
GOVERNANCE_INDEX = ROOT / "schemas/cli/governance-index.json"
# Explicit exception: this code already uses `data.error` as clap-rendered string payload.
TIER0_STRUCTURED_ERROR_EXCEPTIONS = {"argv_parse_error"}


def _load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _object_branch(schema: dict) -> dict:
    if schema.get("type") == "object":
        return schema
    for b in schema.get("anyOf", []):
        if isinstance(b, dict) and b.get("type") == "object":
            return b
    raise ValueError("no object branch found")


def _norm_error_fragment(schema: dict) -> dict:
    keep = {}
    for k in ("type", "additionalProperties", "required", "properties"):
        if k in schema:
            keep[k] = schema[k]
    return keep


def _load_failure_tiers_from_index(index: dict) -> tuple[set[str], set[str]]:
    tiers = index.get("failure_tiers")
    if not isinstance(tiers, dict):
        raise ValueError("schemas/cli/governance-index.json: missing failure_tiers object")
    tier0 = tiers.get("tier0")
    tier1 = tiers.get("tier1")
    if not isinstance(tier0, list) or not all(isinstance(x, str) for x in tier0):
        raise ValueError("schemas/cli/governance-index.json: failure_tiers.tier0 must be string array")
    if not isinstance(tier1, list) or not all(isinstance(x, str) for x in tier1):
        raise ValueError("schemas/cli/governance-index.json: failure_tiers.tier1 must be string array")
    return set(tier0), set(tier1)


def _failure_schema_path(code: str) -> str:
    return f"schemas/cli/data/failure_{code}.json"


def _validate_error_kind_map_instance_schema(instance: dict, schema: dict) -> list[str]:
    bad: list[str] = []
    required = schema.get("required", [])
    props = schema.get("properties", {})
    if not isinstance(required, list) or not isinstance(props, dict):
        return ["schemas/cli/error-kind-map.schema.json: invalid schema structure (required/properties)"]
    for key in required:
        if key not in instance:
            bad.append(f"schemas/cli/error-kind-map.json: missing required field `{key}`")
    for key in instance:
        if key not in props:
            bad.append(f"schemas/cli/error-kind-map.json: unexpected top-level field `{key}`")
    return bad


def _load_error_kind_spec() -> tuple[str, set[str], dict[str, str]]:
    spec = _load(ERROR_KIND_MAP)
    default = spec.get("default")
    kinds = spec.get("kinds")
    mapping = spec.get("map")
    if not isinstance(default, str):
        raise ValueError("schemas/cli/error-kind-map.json: default must be string")
    if not isinstance(kinds, list) or not all(isinstance(x, str) for x in kinds):
        raise ValueError("schemas/cli/error-kind-map.json: kinds must be string array")
    if not isinstance(mapping, dict):
        raise ValueError("schemas/cli/error-kind-map.json: map must be object")
    typed_map: dict[str, str] = {}
    for k, v in mapping.items():
        if not isinstance(k, str) or not isinstance(v, str):
            raise ValueError("schemas/cli/error-kind-map.json: map entries must be string -> string")
        typed_map[k] = v
    return default, set(kinds), typed_map


def _validate_error_kind_contract(fragment: dict, output_rs_src: str, default: str, kinds: set[str], mapping: dict[str, str]) -> list[str]:
    bad: list[str] = []
    kind = fragment.get("properties", {}).get("kind")
    if not isinstance(kind, dict):
        return ["schemas/cli/fragments/error_object.json: missing properties.kind"]
    enum = kind.get("enum")
    if not isinstance(enum, list):
        return ["schemas/cli/fragments/error_object.json: properties.kind.enum must be a list"]
    fragment_kinds = set(x for x in enum if isinstance(x, str))
    if fragment_kinds != kinds:
        bad.append(
            "schemas/cli/fragments/error_object.json: properties.kind.enum differs from schemas/cli/error-kind-map.json kinds"
        )
    if default not in kinds:
        bad.append("schemas/cli/error-kind-map.json: default kind is not in kinds")
    unknown_values = sorted(set(mapping.values()) - kinds)
    if unknown_values:
        bad.append(
            "schemas/cli/error-kind-map.json: map contains values outside kinds: "
            + ", ".join(unknown_values)
        )
    include_marker = 'include_str!("../../../schemas/cli/error-kind-map.json")'
    if include_marker not in output_rs_src:
        bad.append("crates/envr-cli/src/output.rs: must consume schemas/cli/error-kind-map.json via include_str!")
    return bad


def main() -> int:
    base = _load(FRAGMENT)
    kind_map_schema = _load(ERROR_KIND_MAP_SCHEMA)
    kind_map_instance = _load(ERROR_KIND_MAP)
    expected = _norm_error_fragment(base)
    bad: list[str] = []
    bad.extend(_validate_error_kind_map_instance_schema(kind_map_instance, kind_map_schema))
    default, kinds, mapping = _load_error_kind_spec()
    bad.extend(
        _validate_error_kind_contract(
            base,
            OUTPUT_RS.read_text(encoding="utf-8"),
            default,
            kinds,
            mapping,
        )
    )
    _ = OUTPUT_CONTRACT  # documentation is validated by dedicated sync check script.
    index = _load(GOVERNANCE_INDEX)
    tier0, tier1 = _load_failure_tiers_from_index(index)
    # Tier0 policy: require error fragment unless explicitly excepted.
    for code in sorted(tier0):
        rel = _failure_schema_path(code)
        schema = _load(ROOT / rel)
        obj = _object_branch(schema)
        props = obj.get("properties", {})
        got = props.get("error")
        if code in TIER0_STRUCTURED_ERROR_EXCEPTIONS:
            continue
        if not isinstance(got, dict):
            bad.append(f"{rel}: missing properties.error (required for Tier0)")
            continue
        if _norm_error_fragment(got) != expected:
            bad.append(f"{rel}: properties.error differs from schemas/cli/fragments/error_object.json")

    # Tier1 policy: if error exists, it must align with fragment.
    for code in sorted(tier1):
        rel = _failure_schema_path(code)
        schema = _load(ROOT / rel)
        obj = _object_branch(schema)
        props = obj.get("properties", {})
        got = props.get("error")
        if isinstance(got, dict) and _norm_error_fragment(got) != expected:
            bad.append(f"{rel}: optional properties.error differs from schemas/cli/fragments/error_object.json")

    if bad:
        print("schema fragment check failed:")
        for x in bad:
            print(f"  - {x}")
        return 1
    print("schema fragment check: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

