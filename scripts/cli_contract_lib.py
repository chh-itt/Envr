#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path
from typing import Any, Literal

ROOT = Path(__file__).resolve().parents[1]


def run_git(args: list[str], check: bool = True) -> str:
    cp = subprocess.run(
        ["git", *args],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if check and cp.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)} failed: {cp.stderr.strip()}")
    return cp.stdout


def resolve_base_ref(cli_base_ref: str | None) -> str:
    if cli_base_ref:
        return cli_base_ref
    gh_base = os.environ.get("GITHUB_BASE_REF")
    if gh_base:
        return f"origin/{gh_base}"
    return "origin/main"


def resolve_merge_base(base_ref: str) -> str:
    # Prefer merge-base for PR-aware diffs.
    mb = run_git(["merge-base", "HEAD", base_ref], check=False).strip()
    if mb:
        return mb
    # Fallback to fork-point when merge-base is missing in shallow clones.
    mb = run_git(["merge-base", "--fork-point", base_ref, "HEAD"], check=False).strip()
    if mb:
        return mb
    # Last-resort fallback keeps gates running instead of hard-failing on history shape.
    return run_git(["rev-parse", "HEAD~1"], check=False).strip()


def changed_files(merge_base: str) -> list[str]:
    out = run_git(["diff", "--name-only", f"{merge_base}..HEAD"])
    return [line.strip() for line in out.splitlines() if line.strip()]


def changed_cli_schema_files(files: list[str]) -> list[str]:
    return [f for f in files if f.startswith("schemas/cli/") and f.endswith(".json")]


def read_json_from_rev(rev: str, path: str) -> dict[str, Any] | None:
    cp = subprocess.run(
        ["git", "show", f"{rev}:{path}"],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if cp.returncode != 0:
        return None
    try:
        return json.loads(cp.stdout)
    except json.JSONDecodeError:
        return None


def read_json_head(path: str) -> dict[str, Any] | None:
    p = ROOT / path
    if not p.exists():
        return None
    try:
        return json.loads(p.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return None


def read_json_from_rev_state(
    rev: str, path: str
) -> tuple[Literal["ok", "missing", "invalid"], dict[str, Any] | None]:
    cp = subprocess.run(
        ["git", "show", f"{rev}:{path}"],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if cp.returncode != 0:
        return ("missing", None)
    try:
        return ("ok", json.loads(cp.stdout))
    except json.JSONDecodeError:
        return ("invalid", None)


def read_json_head_state(
    path: str,
) -> tuple[Literal["ok", "missing", "invalid"], dict[str, Any] | None]:
    p = ROOT / path
    if not p.exists():
        return ("missing", None)
    try:
        return ("ok", json.loads(p.read_text(encoding="utf-8")))
    except json.JSONDecodeError:
        return ("invalid", None)
