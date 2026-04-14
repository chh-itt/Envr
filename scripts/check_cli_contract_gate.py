#!/usr/bin/env python3
"""
P15 gate: protect CLI contract changes in PRs.

Rules:
1) If `schemas/cli/**` changed, `docs/cli/output-contract.md` must be updated.
2) If schema changes look breaking, output-contract update must include
   `Migration note` and mention changed schema ids/codes.

Usage:
  python scripts/check_cli_contract_gate.py
  python scripts/check_cli_contract_gate.py --base-ref origin/main
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any, Literal

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS_DIR = Path(__file__).resolve().parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))
import cli_contract_lib as lib

OUTPUT_CONTRACT = "docs/cli/output-contract.md"
PORCELAIN_REGRESSION_TEST = "crates/envr-cli/tests/automation_matrix.rs"

PORCELAIN_SENSITIVE_FILES = {
    # Output policy / shared porcelain switch
    "crates/envr-cli/src/output.rs",
    # Commands with documented porcelain contracts
    "crates/envr-cli/src/commands/list.rs",
    "crates/envr-cli/src/commands/current.rs",
    "crates/envr-cli/src/commands/resolve_cmd.rs",
    "crates/envr-cli/src/commands/which.rs",
}
ERROR_KIND_MAP_PATH = "schemas/cli/error-kind-map.json"
GOVERNANCE_INDEX_PATH = "schemas/cli/governance-index.json"


def run_git(args: list[str], check: bool = True) -> str:
    return lib.run_git(args, check=check)


def resolve_base_ref(cli_base_ref: str | None) -> str:
    return lib.resolve_base_ref(cli_base_ref)


def changed_files(merge_base: str) -> list[str]:
    return lib.changed_files(merge_base)


def porcelain_sensitive_changed(files: list[str]) -> list[str]:
    """
    S3a: detect changes likely to impact `--porcelain` contract.
    Returns the subset of changed files that are porcelain-sensitive.
    """
    return [f for f in files if f in PORCELAIN_SENSITIVE_FILES]


def read_json_from_rev(rev: str, path: str) -> dict[str, Any] | None:
    return lib.read_json_from_rev(rev, path)


def read_json_head(path: str) -> dict[str, Any] | None:
    return lib.read_json_head(path)


def read_json_from_rev_state(
    rev: str, path: str
) -> tuple[Literal["ok", "missing", "invalid"], dict[str, Any] | None]:
    return lib.read_json_from_rev_state(rev, path)


def read_json_head_state(
    path: str,
) -> tuple[Literal["ok", "missing", "invalid"], dict[str, Any] | None]:
    return lib.read_json_head_state(path)


def _type_set(v: Any) -> set[str] | None:
    if isinstance(v, str):
        return {v}
    if isinstance(v, list) and all(isinstance(x, str) for x in v):
        return set(v)
    return None


def _as_schema_dict(v: Any) -> dict[str, Any] | None:
    return v if isinstance(v, dict) else None


def _is_breaking_numeric_tightening(old: dict[str, Any], new: dict[str, Any]) -> bool:
    # Minimum family: larger bound is stricter.
    for k in ("minimum", "exclusiveMinimum", "minLength", "minItems", "minProperties"):
        ov = old.get(k)
        nv = new.get(k)
        if isinstance(ov, (int, float)) and isinstance(nv, (int, float)) and nv > ov:
            return True
        if ov is None and isinstance(nv, (int, float)):
            return True
    # Maximum family: smaller bound is stricter.
    for k in ("maximum", "exclusiveMaximum", "maxLength", "maxItems", "maxProperties"):
        ov = old.get(k)
        nv = new.get(k)
        if isinstance(ov, (int, float)) and isinstance(nv, (int, float)) and nv < ov:
            return True
        if ov is None and isinstance(nv, (int, float)):
            return True
    return False


def _append_breaking_numeric_tightening(
    old: dict[str, Any], new: dict[str, Any], path: str, out: list[str]
) -> None:
    for k in ("minimum", "exclusiveMinimum", "minLength", "minItems", "minProperties"):
        ov = old.get(k)
        nv = new.get(k)
        if isinstance(ov, (int, float)) and isinstance(nv, (int, float)) and nv > ov:
            out.append(f"{path}.{k}: tightened bound {ov} -> {nv}")
        if ov is None and isinstance(nv, (int, float)):
            out.append(f"{path}.{k}: introduced bound {nv}")
    for k in ("maximum", "exclusiveMaximum", "maxLength", "maxItems", "maxProperties"):
        ov = old.get(k)
        nv = new.get(k)
        if isinstance(ov, (int, float)) and isinstance(nv, (int, float)) and nv < ov:
            out.append(f"{path}.{k}: tightened bound {ov} -> {nv}")
        if ov is None and isinstance(nv, (int, float)):
            out.append(f"{path}.{k}: introduced bound {nv}")


def _breaking_reason_recursive(old: Any, new: Any, path: str = "$") -> str | None:
    # Boolean-schema handling.
    if isinstance(old, bool):
        if old is False:
            # Old accepts nothing; new cannot break existing valid payloads.
            return None
        # old is True (accept everything): any real restriction is breaking-like.
        if isinstance(new, bool):
            return f"{path}: boolean schema tightened from true to false" if new is False else None
        if isinstance(new, dict):
            return f"{path}: boolean schema tightened from true to object constraints" if len(new) > 0 else None
        return f"{path}: schema shape changed from true to non-schema value"

    if isinstance(new, bool):
        if new is True:
            return None
        # new is False; only non-breaking if old was also False (already handled above)
        return f"{path}: schema tightened to boolean false"

    old_obj = _as_schema_dict(old)
    new_obj = _as_schema_dict(new)
    if old_obj is None or new_obj is None:
        return None

    # Type narrowing.
    old_types = _type_set(old_obj.get("type"))
    new_types = _type_set(new_obj.get("type"))
    if old_types is None and new_types is not None:
        return f"{path}.type: introduced type restriction {sorted(new_types)}"
    if old_types is not None and new_types is not None and not old_types.issubset(new_types):
        return f"{path}.type: narrowed from {sorted(old_types)} to {sorted(new_types)}"

    # required: adding required keys is breaking; removing is not.
    old_required = set(old_obj.get("required", []))
    new_required = set(new_obj.get("required", []))
    if new_required - old_required:
        return f"{path}.required: added required keys {sorted(new_required - old_required)}"

    # enum / const narrowing.
    old_enum = old_obj.get("enum")
    new_enum = new_obj.get("enum")
    if isinstance(old_enum, list) and isinstance(new_enum, list):
        if not set(old_enum).issubset(set(new_enum)):
            removed = sorted(set(old_enum) - set(new_enum))
            return f"{path}.enum: removed values {removed}"
    if old_enum is None and isinstance(new_enum, list):
        return f"{path}.enum: introduced enum restriction"

    old_const = old_obj.get("const")
    new_const = new_obj.get("const")
    if old_const is None and new_const is not None:
        return f"{path}.const: introduced const restriction ({new_const!r})"
    if old_const is not None and new_const is not None and old_const != new_const:
        return f"{path}.const: changed const from {old_const!r} to {new_const!r}"

    if _is_breaking_numeric_tightening(old_obj, new_obj):
        return f"{path}: numeric/string/array/object bounds tightened"

    # String constraints becoming stricter.
    if old_obj.get("pattern") is None and isinstance(new_obj.get("pattern"), str):
        return f"{path}.pattern: introduced regex constraint"
    if old_obj.get("format") is None and isinstance(new_obj.get("format"), str):
        return f"{path}.format: introduced format constraint"

    # Object properties.
    old_props = old_obj.get("properties") or {}
    new_props = new_obj.get("properties") or {}
    if isinstance(old_props, dict) and isinstance(new_props, dict):
        old_additional = old_obj.get("additionalProperties", True)
        for key in old_props:
            if key not in new_props and old_additional is False:
                return f"{path}.properties.{key}: removed property while additionalProperties=false"
        for key in set(old_props.keys()).intersection(new_props.keys()):
            reason = _breaking_reason_recursive(
                old_props[key], new_props[key], f"{path}.properties.{key}"
            )
            if reason:
                return reason
        # New declared property with strict object closure can become breaking.
        if old_obj.get("additionalProperties", True) is False and set(new_props) - set(old_props):
            return (
                f"{path}.properties: added declared keys {sorted(set(new_props) - set(old_props))} "
                "while additionalProperties=false"
            )

    # additionalProperties changes.
    old_ap = old_obj.get("additionalProperties", True)
    new_ap = new_obj.get("additionalProperties", True)
    if old_ap is True and new_ap is False:
        return f"{path}.additionalProperties: tightened from true to false"
    if isinstance(old_ap, dict):
        if new_ap is False:
            return f"{path}.additionalProperties: tightened from schema to false"
        if isinstance(new_ap, dict) and _breaking_reason_recursive(old_ap, new_ap) is not None:
            reason = _breaking_reason_recursive(old_ap, new_ap, f"{path}.additionalProperties")
            if reason:
                return reason
    if old_ap is True and isinstance(new_ap, dict):
        return f"{path}.additionalProperties: tightened from true to schema"

    # Array item schemas.
    if "items" in old_obj and "items" in new_obj:
        reason = _breaking_reason_recursive(old_obj["items"], new_obj["items"], f"{path}.items")
        if reason:
            return reason
    elif "items" not in old_obj and "items" in new_obj:
        return f"{path}.items: introduced item schema restriction"

    for key in ("prefixItems",):
        ov = old_obj.get(key)
        nv = new_obj.get(key)
        if isinstance(ov, list) and isinstance(nv, list):
            for o_item, n_item in zip(ov, nv):
                reason = _breaking_reason_recursive(o_item, n_item, f"{path}.{key}[]")
                if reason:
                    return reason
            if len(nv) > len(ov):
                return f"{path}.{key}: introduced additional positional item constraints"
        elif ov is None and isinstance(nv, list) and nv:
            return f"{path}.{key}: introduced positional item constraints"

    # Composition keywords: adding branches is generally stricter (breaking-like).
    for key in ("allOf", "anyOf", "oneOf"):
        ov = old_obj.get(key)
        nv = new_obj.get(key)
        if ov is None and isinstance(nv, list) and nv:
            return f"{path}.{key}: introduced composition constraints"
        if isinstance(ov, list) and isinstance(nv, list):
            if len(nv) > len(ov):
                return f"{path}.{key}: increased branch count ({len(ov)} -> {len(nv)})"
            for o_item, n_item in zip(ov, nv):
                reason = _breaking_reason_recursive(o_item, n_item, f"{path}.{key}[]")
                if reason:
                    return reason

    return None


def is_breaking_schema_change(old: dict[str, Any], new: dict[str, Any]) -> bool:
    return _breaking_reason_recursive(old, new) is not None


def first_breaking_reason(old: dict[str, Any], new: dict[str, Any]) -> str | None:
    return _breaking_reason_recursive(old, new)


def _collect_breaking_reasons(old: Any, new: Any, path: str, out: list[str]) -> None:
    if isinstance(old, bool):
        if old is False:
            return
        if isinstance(new, bool):
            if new is False:
                out.append(f"{path}: boolean schema tightened from true to false")
            return
        if isinstance(new, dict):
            if len(new) > 0:
                out.append(f"{path}: boolean schema tightened from true to object constraints")
            return
        out.append(f"{path}: schema shape changed from true to non-schema value")
        return
    if isinstance(new, bool):
        if new is False:
            out.append(f"{path}: schema tightened to boolean false")
        return

    old_obj = _as_schema_dict(old)
    new_obj = _as_schema_dict(new)
    if old_obj is None or new_obj is None:
        return

    old_types = _type_set(old_obj.get("type"))
    new_types = _type_set(new_obj.get("type"))
    if old_types is None and new_types is not None:
        out.append(f"{path}.type: introduced type restriction {sorted(new_types)}")
    if old_types is not None and new_types is not None and not old_types.issubset(new_types):
        out.append(f"{path}.type: narrowed from {sorted(old_types)} to {sorted(new_types)}")

    old_required = set(old_obj.get("required", []))
    new_required = set(new_obj.get("required", []))
    if new_required - old_required:
        out.append(f"{path}.required: added required keys {sorted(new_required - old_required)}")

    old_enum = old_obj.get("enum")
    new_enum = new_obj.get("enum")
    if isinstance(old_enum, list) and isinstance(new_enum, list):
        removed = sorted(set(old_enum) - set(new_enum))
        if removed:
            out.append(f"{path}.enum: removed values {removed}")
    if old_enum is None and isinstance(new_enum, list):
        out.append(f"{path}.enum: introduced enum restriction")

    old_const = old_obj.get("const")
    new_const = new_obj.get("const")
    if old_const is None and new_const is not None:
        out.append(f"{path}.const: introduced const restriction ({new_const!r})")
    if old_const is not None and new_const is not None and old_const != new_const:
        out.append(f"{path}.const: changed const from {old_const!r} to {new_const!r}")

    _append_breaking_numeric_tightening(old_obj, new_obj, path, out)

    if old_obj.get("pattern") is None and isinstance(new_obj.get("pattern"), str):
        out.append(f"{path}.pattern: introduced regex constraint")
    if old_obj.get("format") is None and isinstance(new_obj.get("format"), str):
        out.append(f"{path}.format: introduced format constraint")

    old_props = old_obj.get("properties") or {}
    new_props = new_obj.get("properties") or {}
    if isinstance(old_props, dict) and isinstance(new_props, dict):
        old_additional = old_obj.get("additionalProperties", True)
        for key in old_props:
            if key not in new_props and old_additional is False:
                out.append(f"{path}.properties.{key}: removed property while additionalProperties=false")
        for key in set(old_props.keys()).intersection(new_props.keys()):
            _collect_breaking_reasons(old_props[key], new_props[key], f"{path}.properties.{key}", out)
        added_keys = sorted(set(new_props) - set(old_props))
        if old_obj.get("additionalProperties", True) is False and added_keys:
            out.append(f"{path}.properties: added declared keys {added_keys} while additionalProperties=false")

    old_ap = old_obj.get("additionalProperties", True)
    new_ap = new_obj.get("additionalProperties", True)
    if old_ap is True and new_ap is False:
        out.append(f"{path}.additionalProperties: tightened from true to false")
    if isinstance(old_ap, dict):
        if new_ap is False:
            out.append(f"{path}.additionalProperties: tightened from schema to false")
        if isinstance(new_ap, dict):
            _collect_breaking_reasons(old_ap, new_ap, f"{path}.additionalProperties", out)
    if old_ap is True and isinstance(new_ap, dict):
        out.append(f"{path}.additionalProperties: tightened from true to schema")

    if "items" in old_obj and "items" in new_obj:
        _collect_breaking_reasons(old_obj["items"], new_obj["items"], f"{path}.items", out)
    elif "items" not in old_obj and "items" in new_obj:
        out.append(f"{path}.items: introduced item schema restriction")

    ov = old_obj.get("prefixItems")
    nv = new_obj.get("prefixItems")
    if isinstance(ov, list) and isinstance(nv, list):
        for o_item, n_item in zip(ov, nv):
            _collect_breaking_reasons(o_item, n_item, f"{path}.prefixItems[]", out)
        if len(nv) > len(ov):
            out.append(f"{path}.prefixItems: introduced additional positional item constraints")
    elif ov is None and isinstance(nv, list) and nv:
        out.append(f"{path}.prefixItems: introduced positional item constraints")

    for key in ("allOf", "anyOf", "oneOf"):
        ov = old_obj.get(key)
        nv = new_obj.get(key)
        if ov is None and isinstance(nv, list) and nv:
            out.append(f"{path}.{key}: introduced composition constraints")
        if isinstance(ov, list) and isinstance(nv, list):
            if len(nv) > len(ov):
                out.append(f"{path}.{key}: increased branch count ({len(ov)} -> {len(nv)})")
            for o_item, n_item in zip(ov, nv):
                _collect_breaking_reasons(o_item, n_item, f"{path}.{key}[]", out)


def all_breaking_reasons(old: dict[str, Any], new: dict[str, Any]) -> list[str]:
    out: list[str] = []
    _collect_breaking_reasons(old, new, "$", out)
    # preserve order, dedupe
    seen: set[str] = set()
    uniq: list[str] = []
    for r in out:
        if r not in seen:
            seen.add(r)
            uniq.append(r)
    return uniq


def error_kind_map_breaking_reasons(old: dict[str, Any], new: dict[str, Any]) -> list[str]:
    """
    P40.6: semantic breaking checks for error-kind-map.
    """
    out: list[str] = []
    old_kinds = set(old.get("kinds", [])) if isinstance(old.get("kinds"), list) else set()
    new_kinds = set(new.get("kinds", [])) if isinstance(new.get("kinds"), list) else set()
    if old_kinds and new_kinds:
        removed_kinds = sorted(old_kinds - new_kinds)
        if removed_kinds:
            out.append(f"$.kinds: removed kinds {removed_kinds}")
    old_default = old.get("default")
    new_default = new.get("default")
    if isinstance(old_default, str) and isinstance(new_default, str) and old_default != new_default:
        out.append(f"$.default: changed from {old_default!r} to {new_default!r}")
    old_map = old.get("map", {}) if isinstance(old.get("map"), dict) else {}
    new_map = new.get("map", {}) if isinstance(new.get("map"), dict) else {}
    removed_codes = sorted(set(old_map.keys()) - set(new_map.keys()))
    if removed_codes:
        out.append(f"$.map: removed mapped codes {removed_codes}")
    for code in sorted(set(old_map.keys()).intersection(new_map.keys())):
        ov = old_map.get(code)
        nv = new_map.get(code)
        if isinstance(ov, str) and isinstance(nv, str) and ov != nv:
            out.append(f"$.map.{code}: changed from {ov!r} to {nv!r}")
    return out


def error_kind_map_change_summary(old: dict[str, Any], new: dict[str, Any]) -> dict[str, Any]:
    old_kinds = set(old.get("kinds", [])) if isinstance(old.get("kinds"), list) else set()
    new_kinds = set(new.get("kinds", [])) if isinstance(new.get("kinds"), list) else set()
    removed_kinds = sorted(old_kinds - new_kinds)

    old_default = old.get("default") if isinstance(old.get("default"), str) else None
    new_default = new.get("default") if isinstance(new.get("default"), str) else None
    default_changed = None
    if old_default is not None and new_default is not None and old_default != new_default:
        default_changed = {"from": old_default, "to": new_default}

    old_map = old.get("map", {}) if isinstance(old.get("map"), dict) else {}
    new_map = new.get("map", {}) if isinstance(new.get("map"), dict) else {}
    removed_codes = sorted(set(old_map.keys()) - set(new_map.keys()))
    remapped_codes: list[dict[str, str]] = []
    for code in sorted(set(old_map.keys()).intersection(new_map.keys())):
        ov = old_map.get(code)
        nv = new_map.get(code)
        if isinstance(ov, str) and isinstance(nv, str) and ov != nv:
            remapped_codes.append({"code": code, "from": ov, "to": nv})
    return {
        "removed_kinds": removed_kinds,
        "default_changed": default_changed,
        "removed_codes": removed_codes,
        "remapped_codes": remapped_codes,
    }


def analyze_error_kind_map_change(old: dict[str, Any], new: dict[str, Any]) -> dict[str, Any]:
    summary = error_kind_map_change_summary(old, new)
    reasons = error_kind_map_breaking_reasons(old, new)
    return {
        "breaking": bool(reasons),
        "reasons": reasons,
        "summary": summary,
        "migration_note_hint": error_kind_map_migration_note_hint(summary),
    }


def governance_index_breaking_reasons(old: dict[str, Any], new: dict[str, Any]) -> list[str]:
    out: list[str] = []
    old_tiers = old.get("failure_tiers", {}) if isinstance(old.get("failure_tiers"), dict) else {}
    new_tiers = new.get("failure_tiers", {}) if isinstance(new.get("failure_tiers"), dict) else {}
    for tier in ("tier0", "tier1", "tier2"):
        old_codes = set(old_tiers.get(tier, [])) if isinstance(old_tiers.get(tier), list) else set()
        new_codes = set(new_tiers.get(tier, [])) if isinstance(new_tiers.get(tier), list) else set()
        removed = sorted(old_codes - new_codes)
        if removed:
            out.append(f"$.failure_tiers.{tier}: removed codes {removed}")

    old_porcelain = (
        old.get("porcelain_matrix_rows", {})
        if isinstance(old.get("porcelain_matrix_rows"), dict)
        else {}
    )
    new_porcelain = (
        new.get("porcelain_matrix_rows", {})
        if isinstance(new.get("porcelain_matrix_rows"), dict)
        else {}
    )
    for cmd in sorted(set(old_porcelain.keys()) - set(new_porcelain.keys())):
        out.append(f"$.porcelain_matrix_rows: removed command mapping {cmd!r}")
    for cmd in sorted(set(old_porcelain.keys()).intersection(new_porcelain.keys())):
        ov = old_porcelain.get(cmd)
        nv = new_porcelain.get(cmd)
        if not isinstance(ov, dict) or not isinstance(nv, dict):
            continue
        old_expected = ov.get("porcelain_expected")
        new_expected = nv.get("porcelain_expected")
        if old_expected is True and new_expected is False:
            out.append(f"$.porcelain_matrix_rows.{cmd}.porcelain_expected: changed true -> false")

    old_offline = (
        old.get("offline_coverage_rows", {})
        if isinstance(old.get("offline_coverage_rows"), dict)
        else {}
    )
    new_offline = (
        new.get("offline_coverage_rows", {})
        if isinstance(new.get("offline_coverage_rows"), dict)
        else {}
    )
    for cmd in sorted(set(old_offline.keys()) - set(new_offline.keys())):
        out.append(f"$.offline_coverage_rows: removed command mapping {cmd!r}")
    for cmd in sorted(set(old_offline.keys()).intersection(new_offline.keys())):
        ov = old_offline.get(cmd)
        nv = new_offline.get(cmd)
        if not isinstance(ov, dict) or not isinstance(nv, dict):
            continue
        old_skip = ov.get("network_skip_allowed")
        new_skip = nv.get("network_skip_allowed")
        if old_skip is False and new_skip is True:
            out.append(f"$.offline_coverage_rows.{cmd}.network_skip_allowed: changed false -> true")
    return out


def governance_index_change_summary(old: dict[str, Any], new: dict[str, Any]) -> dict[str, Any]:
    old_tiers = old.get("failure_tiers", {}) if isinstance(old.get("failure_tiers"), dict) else {}
    new_tiers = new.get("failure_tiers", {}) if isinstance(new.get("failure_tiers"), dict) else {}
    removed_tier_codes: dict[str, list[str]] = {}
    for tier in ("tier0", "tier1", "tier2"):
        old_codes = set(old_tiers.get(tier, [])) if isinstance(old_tiers.get(tier), list) else set()
        new_codes = set(new_tiers.get(tier, [])) if isinstance(new_tiers.get(tier), list) else set()
        removed = sorted(old_codes - new_codes)
        if removed:
            removed_tier_codes[tier] = removed

    old_porcelain = (
        old.get("porcelain_matrix_rows", {})
        if isinstance(old.get("porcelain_matrix_rows"), dict)
        else {}
    )
    new_porcelain = (
        new.get("porcelain_matrix_rows", {})
        if isinstance(new.get("porcelain_matrix_rows"), dict)
        else {}
    )
    removed_porcelain_commands = sorted(set(old_porcelain.keys()) - set(new_porcelain.keys()))
    downgraded_porcelain_commands: list[str] = []
    for cmd in sorted(set(old_porcelain.keys()).intersection(new_porcelain.keys())):
        ov = old_porcelain.get(cmd)
        nv = new_porcelain.get(cmd)
        if not isinstance(ov, dict) or not isinstance(nv, dict):
            continue
        if ov.get("porcelain_expected") is True and nv.get("porcelain_expected") is False:
            downgraded_porcelain_commands.append(cmd)

    old_offline = (
        old.get("offline_coverage_rows", {})
        if isinstance(old.get("offline_coverage_rows"), dict)
        else {}
    )
    new_offline = (
        new.get("offline_coverage_rows", {})
        if isinstance(new.get("offline_coverage_rows"), dict)
        else {}
    )
    removed_offline_commands = sorted(set(old_offline.keys()) - set(new_offline.keys()))
    relaxed_offline_network_skip_commands: list[str] = []
    for cmd in sorted(set(old_offline.keys()).intersection(new_offline.keys())):
        ov = old_offline.get(cmd)
        nv = new_offline.get(cmd)
        if not isinstance(ov, dict) or not isinstance(nv, dict):
            continue
        if ov.get("network_skip_allowed") is False and nv.get("network_skip_allowed") is True:
            relaxed_offline_network_skip_commands.append(cmd)

    return {
        "removed_tier_codes": removed_tier_codes,
        "removed_porcelain_commands": removed_porcelain_commands,
        "downgraded_porcelain_commands": downgraded_porcelain_commands,
        "removed_offline_commands": removed_offline_commands,
        "relaxed_offline_network_skip_commands": relaxed_offline_network_skip_commands,
    }


def governance_index_migration_note_hint(summary: dict[str, Any]) -> str:
    bits: list[str] = []
    removed_tier_codes = summary.get("removed_tier_codes", {})
    if isinstance(removed_tier_codes, dict) and removed_tier_codes:
        bits.append(f"removed tier codes={removed_tier_codes}")
    removed_porcelain = summary.get("removed_porcelain_commands", [])
    if removed_porcelain:
        bits.append(f"removed porcelain mappings={removed_porcelain}")
    downgraded_porcelain = summary.get("downgraded_porcelain_commands", [])
    if downgraded_porcelain:
        bits.append(f"porcelain downgraded={downgraded_porcelain}")
    removed_offline = summary.get("removed_offline_commands", [])
    if removed_offline:
        bits.append(f"removed offline mappings={removed_offline}")
    relaxed_skip = summary.get("relaxed_offline_network_skip_commands", [])
    if relaxed_skip:
        bits.append(f"offline network-skip relaxed={relaxed_skip}")
    detail = "; ".join(bits) if bits else "no semantic diff details available"
    return (
        "Migration note: governance_index changed. "
        f"{detail}. "
        "Document automation policy impact and downstream fallback behavior."
    )


def analyze_governance_index_change(old: dict[str, Any], new: dict[str, Any]) -> dict[str, Any]:
    summary = governance_index_change_summary(old, new)
    reasons = governance_index_breaking_reasons(old, new)
    return {
        "breaking": bool(reasons),
        "reasons": reasons,
        "summary": summary,
        "migration_note_hint": governance_index_migration_note_hint(summary),
    }


def first_breaking_reason_for_path(path: str, old: dict[str, Any], new: dict[str, Any]) -> str | None:
    if path == ERROR_KIND_MAP_PATH:
        reasons = analyze_error_kind_map_change(old, new)["reasons"]
        if reasons:
            return reasons[0]
    if path == GOVERNANCE_INDEX_PATH:
        reasons = analyze_governance_index_change(old, new)["reasons"]
        if reasons:
            return reasons[0]
    return first_breaking_reason(old, new)


def all_breaking_reasons_for_path(path: str, old: dict[str, Any], new: dict[str, Any]) -> list[str]:
    if path == ERROR_KIND_MAP_PATH:
        return analyze_error_kind_map_change(old, new)["reasons"]
    if path == GOVERNANCE_INDEX_PATH:
        return analyze_governance_index_change(old, new)["reasons"]
    return all_breaking_reasons(old, new)


def error_kind_map_migration_note_hint(summary: dict[str, Any]) -> str:
    bits: list[str] = []
    removed_kinds = summary.get("removed_kinds", [])
    if removed_kinds:
        bits.append(f"removed kinds={removed_kinds}")
    default_changed = summary.get("default_changed")
    if isinstance(default_changed, dict):
        bits.append(f"default {default_changed.get('from')!r}->{default_changed.get('to')!r}")
    removed_codes = summary.get("removed_codes", [])
    if removed_codes:
        bits.append(f"removed mapped codes={removed_codes}")
    remapped_codes = summary.get("remapped_codes", [])
    if remapped_codes:
        bits.append(f"remapped codes={remapped_codes}")
    detail = "; ".join(bits) if bits else "no semantic diff details available"
    return (
        "Migration note: error_kind_map changed. "
        f"{detail}. "
        "Document impacted failure codes and downstream parser fallback behavior."
    )


def run_self_tests() -> int:
    # non-breaking: optional property added
    old = {"type": "object", "properties": {"a": {"type": "string"}}}
    new = {
        "type": "object",
        "properties": {"a": {"type": "string"}, "b": {"type": "string"}},
    }
    assert not is_breaking_schema_change(old, new)

    # breaking: required field added
    old = {"type": "object", "required": ["a"], "properties": {"a": {"type": "string"}}}
    new = {
        "type": "object",
        "required": ["a", "b"],
        "properties": {"a": {"type": "string"}, "b": {"type": "string"}},
    }
    assert is_breaking_schema_change(old, new)

    # breaking: enum narrowed
    old = {"enum": ["a", "b", "c"]}
    new = {"enum": ["a", "b"]}
    assert is_breaking_schema_change(old, new)

    # breaking: nested items tightened
    old = {
        "type": "array",
        "items": {"type": "object", "properties": {"v": {"type": "string"}}},
    }
    new = {
        "type": "array",
        "items": {
            "type": "object",
            "properties": {"v": {"type": "string", "minLength": 3}},
        },
    }
    assert is_breaking_schema_change(old, new)

    # breaking: additionalProperties true -> false
    old = {"type": "object", "additionalProperties": True}
    new = {"type": "object", "additionalProperties": False}
    assert is_breaking_schema_change(old, new)

    # non-breaking: required removed
    old = {"type": "object", "required": ["a", "b"]}
    new = {"type": "object", "required": ["a"]}
    assert not is_breaking_schema_change(old, new)

    ids = changed_schema_ids(
        [
            "schemas/cli/data/failure_child_exit.json",
            "schemas/cli/data/config_set.json",
            "schemas/cli/index.json",
        ]
    )
    assert ids == {"child_exit", "config_set", "index"}
    assert migration_note_mentions_all_ids(
        "+ Migration note: touched child_exit and config_set and index.\n", ids
    )
    assert not migration_note_mentions_all_ids(
        "+ Migration note: only child_exit.\n", ids
    )
    reasons = all_breaking_reasons(
        {"type": "object", "properties": {"a": {"type": "string"}}},
        {
            "type": "object",
            "required": ["a"],
            "properties": {"a": {"type": "string", "minLength": 2}},
        },
    )
    assert len(reasons) >= 2
    assert any(".required" in r for r in reasons)
    assert any(".minLength" in r for r in reasons)

    print("contract gate self-test: ok")
    return 0


def output_contract_added_lines(merge_base: str) -> str:
    diff = run_git(["diff", "--unified=0", f"{merge_base}..HEAD", "--", OUTPUT_CONTRACT], check=False)
    lines: list[str] = []
    for line in diff.splitlines():
        if line.startswith("+++") or line.startswith("@@"):
            continue
        if line.startswith("+"):
            lines.append(line[1:])
    return "\n".join(lines)


def changed_schema_ids(paths: list[str]) -> set[str]:
    ids: set[str] = set()
    for path in paths:
        stem = Path(path).stem
        if stem.startswith("failure_"):
            ids.add(stem[len("failure_") :])
        else:
            ids.add(stem)
    return ids


def migration_note_mentions_all_ids(added_text: str, ids: set[str]) -> bool:
    lowered = added_text.lower()
    if "migration note" not in lowered:
        return False
    return all(token.lower() in lowered for token in ids)


def recommend_bumps(schema_changed: list[str], breaking_schema_files: list[str]) -> dict[str, Any]:
    """
    P34: heuristics to suggest version bumps / release actions.

    This is advisory only; it does not gate PRs.
    """
    recommended_bumps: list[str] = []
    release_actions: list[str] = []

    any_schema_changed = bool(schema_changed)
    any_breaking = bool(breaking_schema_files)
    any_data_schema_changed = any(p.startswith("schemas/cli/data/") for p in schema_changed)
    any_metrics_changed = "schemas/cli/metrics-event.json" in schema_changed

    if any_schema_changed:
        release_actions.append("review_cli_contract_report_artifact")
        release_actions.append("ensure_output_contract_docs_updated")

    if any_breaking:
        release_actions.append("add_migration_note")
        # repo mirrors under schemas/cli/** represent script-facing contract surface; breaking-like
        # changes should generally bump the top-level schema version used by JSON envelopes.
        recommended_bumps.append("cli_json_schema_version")

    if any_data_schema_changed:
        release_actions.append("run_envr_cli_schema_contract_tests")

    if any_metrics_changed:
        release_actions.append("review_metrics_schema_and_docs")

    # stable ordering / dedupe
    def uniq(xs: list[str]) -> list[str]:
        seen: set[str] = set()
        out: list[str] = []
        for x in xs:
            if x not in seen:
                seen.add(x)
                out.append(x)
        return out

    return {
        "recommended_bumps": uniq(recommended_bumps),
        "release_actions": uniq(release_actions),
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--base-ref", default=None, help="base branch/ref (default: origin/main or GITHUB_BASE_REF)")
    ap.add_argument("--self-test", action="store_true", help="run built-in detector test cases")
    ap.add_argument("--explain", action="store_true", help="print breaking-detection reasons")
    args = ap.parse_args()

    if args.self_test:
        return run_self_tests()

    base_ref = resolve_base_ref(args.base_ref)
    merge_base = lib.resolve_merge_base(base_ref)
    if not merge_base:
        print(f"contract gate: unable to resolve merge-base with {base_ref}", file=sys.stderr)
        return 2

    files = changed_files(merge_base)

    porcelain_changed = porcelain_sensitive_changed(files)
    if porcelain_changed:
        if OUTPUT_CONTRACT not in files and PORCELAIN_REGRESSION_TEST not in files:
            print(
                "contract gate failed: porcelain-sensitive code changed but neither "
                "docs/cli/output-contract.md nor crates/envr-cli/tests/automation_matrix.rs was updated.",
                file=sys.stderr,
            )
            for p in porcelain_changed:
                print(f"  - {p}", file=sys.stderr)
            return 1
    schema_changed = [f for f in files if f.startswith("schemas/cli/") and f.endswith(".json")]
    if not schema_changed:
        print("contract gate: no schema changes detected")
        return 0

    if OUTPUT_CONTRACT not in files:
        print(
            "contract gate failed: schema changed but docs/cli/output-contract.md was not updated.",
            file=sys.stderr,
        )
        return 1

    breaking_files: list[str] = []
    breaking_reasons: dict[str, str] = {}
    unreadable_files: list[str] = []
    for path in schema_changed:
        old_state, old = read_json_from_rev_state(merge_base, path)
        new_state, new = read_json_head_state(path)
        if old_state == "invalid" or new_state == "invalid":
            unreadable_files.append(path)
            continue
        if old_state == "missing" and new_state == "missing":
            unreadable_files.append(path)
            continue
        if old_state == "missing" and new_state == "ok":
            # New schema file is additive by default for gating.
            continue
        if old_state == "ok" and new_state == "missing":
            # Deleting an existing schema contract is breaking-like.
            breaking_files.append(path)
            breaking_reasons[path] = "schema file deleted from HEAD"
            continue
        if old is not None and new is not None:
            reason = first_breaking_reason_for_path(path, old, new)
            if reason:
                breaking_files.append(path)
                breaking_reasons[path] = reason

    if unreadable_files:
        print(
            "contract gate failed: unable to parse/read changed schema JSON files (fail-closed):",
            file=sys.stderr,
        )
        for p in unreadable_files:
            print(f"  - {p}", file=sys.stderr)
        return 1

    if breaking_files:
        added_lines = output_contract_added_lines(merge_base)
        ids = changed_schema_ids(breaking_files)
        if migration_note_mentions_all_ids(added_lines, ids):
            print("contract gate: ok")
            return 0
        print("contract gate failed: breaking-like schema change detected:", file=sys.stderr)
        for p in breaking_files:
            print(f"  - {p}", file=sys.stderr)
            if args.explain:
                old = read_json_from_rev(merge_base, p) or {}
                new = read_json_head(p) or {}
                reasons = all_breaking_reasons_for_path(p, old, new)
                if reasons:
                    for r in reasons:
                        print(f"    reason: {r}", file=sys.stderr)
                else:
                    print(f"    reason: {breaking_reasons.get(p, 'unknown')}", file=sys.stderr)
                if p == ERROR_KIND_MAP_PATH and isinstance(old, dict) and isinstance(new, dict):
                    analysis = analyze_error_kind_map_change(old, new)
                    summary = analysis["summary"]
                    print(f"    error_kind_map_summary: {json.dumps(summary, ensure_ascii=False)}", file=sys.stderr)
                    print(
                        f"    migration_note_hint: {analysis['migration_note_hint']}",
                        file=sys.stderr,
                    )
                if p == GOVERNANCE_INDEX_PATH and isinstance(old, dict) and isinstance(new, dict):
                    analysis = analyze_governance_index_change(old, new)
                    summary = analysis["summary"]
                    print(
                        f"    governance_index_summary: {json.dumps(summary, ensure_ascii=False)}",
                        file=sys.stderr,
                    )
                    print(
                        f"    migration_note_hint: {analysis['migration_note_hint']}",
                        file=sys.stderr,
                    )
        print(
            "please update docs/cli/output-contract.md with `Migration note` and mention all changed ids/codes: "
            + ", ".join(sorted(ids)),
            file=sys.stderr,
        )
        return 1

    print("contract gate: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

