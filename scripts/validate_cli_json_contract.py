#!/usr/bin/env python3
"""
Validate envr --format json envelopes + data against docs/schemas.

Requires: pip install jsonschema referencing

Usage (from repo root):
  cargo build -p envr-cli -q
  python scripts/validate_cli_json_contract.py

Override binary:
  ENVR_JSON_VALIDATE_BIN=path/to/envr python scripts/validate_cli_json_contract.py
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

try:
    from jsonschema import Draft202012Validator
except ImportError:
    print("Install dependencies: pip install jsonschema referencing", file=sys.stderr)
    sys.exit(2)

ROOT = Path(__file__).resolve().parents[1]
SCHEMA_DIR = ROOT / "docs" / "schemas"

ENVELOPE_SCHEMA = SCHEMA_DIR / "cli-envelope-v2.schema.json"

# message -> data schema filename (success, non-null data)
MESSAGE_SCHEMA: dict[str, str] = {
    "list_installed": "data-list-installed-v1.schema.json",
    "list_remote": "data-list-remote-v1.schema.json",
    "show_current": "data-show-current-v1.schema.json",
    "doctor_ok": "data-doctor-v2.schema.json",
    "doctor_issues": "data-doctor-v2.schema.json",
    "deactivate_hint": "data-deactivate-hint-v1.schema.json",
    "project_status": "data-project-status-v1.schema.json",
    "hook_prompt": "data-hook-prompt-v1.schema.json",
    "template_rendered": "data-template-rendered-v1.schema.json",
    "installed": "data-runtime-kind-version-v1.schema.json",
    "uninstalled": "data-runtime-kind-version-v1.schema.json",
    "current_runtime_set": "data-runtime-kind-version-v1.schema.json",
    "child_completed": "data-child-process-v1.schema.json",
    "child_exit": "data-child-process-v1.schema.json",
    "project_config_ok": "data-project-config-ok-v1.schema.json",
    "runtime_resolved": "data-runtime-resolved-v1.schema.json",
    "resolved_executable": "data-resolved-executable-v1.schema.json",
    "config_path": "data-config-path-v1.schema.json",
    "config_keys": "data-config-keys-v1.schema.json",
    "config_get": "data-config-get-v1.schema.json",
    "config_set": "data-config-set-v1.schema.json",
    "config_show": "data-config-show-v1.schema.json",
    "bundle_created": "data-bundle-created-v1.schema.json",
    "bundle_applied": "data-bundle-applied-v1.schema.json",
    "prune_dry_run": "data-prune-dry-run-v1.schema.json",
    "prune_executed": "data-prune-executed-v1.schema.json",
    "alias_list": "data-alias-list-v1.schema.json",
    "alias_added": "data-alias-added-v1.schema.json",
    "alias_removed": "data-alias-removed-v1.schema.json",
    "profiles_list": "data-profiles-list-v1.schema.json",
    "profile_show": "data-profile-show-v1.schema.json",
    "project_pin_added": "data-project-op-v1.schema.json",
    "project_synced": "data-project-op-v1.schema.json",
    "project_sync_pending": "data-project-op-v1.schema.json",
    "project_validated": "data-project-op-v1.schema.json",
    "project_validate_failed": "data-project-op-v1.schema.json",
    "project_config_init": "data-project-config-init-v1.schema.json",
    "project_env": "data-project-env-v1.schema.json",
    "update_info": "data-update-info-v1.schema.json",
    "cache_cleaned": "data-cache-cleaned-v1.schema.json",
    "cache_index_synced": "data-cache-index-synced-v1.schema.json",
    "cache_index_status": "data-cache-index-status-v1.schema.json",
    "config_imported": "data-config-imported-v1.schema.json",
    "config_exported": "data-config-exported-v1.schema.json",
    "shims_synced": "data-shims-synced-v1.schema.json",
    "diagnostics_export_ok": "data-diagnostics-export-ok-v1.schema.json",
}

def load_schema(name: str) -> dict:
    path = SCHEMA_DIR / name
    with path.open(encoding="utf-8") as f:
        return json.load(f)


def validate(instance: dict, schema: dict, label: str) -> None:
    Draft202012Validator.check_schema(schema)
    v = Draft202012Validator(schema)
    errors = sorted(v.iter_errors(instance), key=lambda e: e.path)
    if errors:
        msg = "\n".join(f"  {e.json_path}: {e.message}" for e in errors[:12])
        raise AssertionError(f"{label} validation failed:\n{msg}")


def find_envr() -> Path:
    override = os.environ.get("ENVR_JSON_VALIDATE_BIN")
    if override:
        return Path(override)
    name = "envr.exe" if os.name == "nt" else "envr"
    debug = ROOT / "target" / "debug" / name
    if debug.is_file():
        return debug
    return Path(name)


def run_envr(envr: Path, runtime_root: Path, args: list[str]) -> subprocess.CompletedProcess:
    env = os.environ.copy()
    env["ENVR_RUNTIME_ROOT"] = str(runtime_root)
    return subprocess.run(
        [str(envr), *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        env=env,
        check=False,
    )


def parse_json_line(stdout: str | None) -> dict:
    if stdout is None:
        raise ValueError("empty stdout")
    for line in stdout.splitlines():
        line = line.strip()
        if line.startswith("{"):
            return json.loads(line)
    raise ValueError(f"no JSON object in stdout: {stdout[:500]!r}")


def main() -> int:
    envr = find_envr()
    envelope_schema = load_schema(ENVELOPE_SCHEMA.name)
    null_schema = load_schema("data-null-v1.schema.json")

    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        (root / "t.tpl").write_text('{"p":"${PATH}"}\n', encoding="utf-8")
        cases: list[tuple[str, list[str], bool]] = [
            ("update", ["--format", "json", "update"], True),
            ("list", ["--format", "json", "list"], True),
            ("current", ["--format", "json", "current"], True),
            ("doctor", ["--format", "json", "doctor"], True),
            ("doctor_json_flag", ["doctor", "--json"], True),
            ("deactivate", ["--format", "json", "deactivate"], True),
            ("status", ["--format", "json", "status"], True),
            ("hook_prompt", ["--format", "json", "hook", "prompt"], True),
            (
                "project_add",
                ["--format", "json", "project", "add", "node@20", "--path", str(root)],
                True,
            ),
            (
                "template",
                ["--format", "json", "template", str(root / "t.tpl")],
                True,
            ),
            (
                "run_child",
                (
                    ["--format", "json", "run", "cmd", "/c", "echo", "ok"]
                    if os.name == "nt"
                    else ["--format", "json", "run", "sh", "-c", "echo ok"]
                ),
                True,
            ),
            ("list_invalid", ["--format", "json", "list", "not-a-lang"], False),
        ]
        for name, argv, expect_ok in cases:
            cp = run_envr(envr, root, argv)
            if expect_ok and cp.returncode != 0:
                print(f"FAIL {name}: exit {cp.returncode} stderr={cp.stderr!r}", file=sys.stderr)
                return 1
            if not expect_ok and cp.returncode == 0:
                print(f"FAIL {name}: expected failure", file=sys.stderr)
                return 1
            try:
                payload = parse_json_line(cp.stdout)
            except ValueError as e:
                print(f"FAIL {name}: {e}", file=sys.stderr)
                return 1

            validate(payload, envelope_schema, f"{name} envelope")

            data = payload.get("data")
            if data is None:
                validate(data, null_schema, f"{name} data null")
                continue
            msg = payload.get("message")
            if not isinstance(msg, str) or msg not in MESSAGE_SCHEMA:
                print(
                    f"SKIP {name}: no data schema for message={msg!r}",
                    file=sys.stderr,
                )
                continue
            schema_name = MESSAGE_SCHEMA[msg]
            data_schema = load_schema(schema_name)
            validate(data, data_schema, f"{name} data ({schema_name})")

    print("ok: cli json contract samples validated")
    return 0


if __name__ == "__main__":
    sys.exit(main())
