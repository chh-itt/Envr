#!/usr/bin/env python3
"""Lightweight governance checks for RuntimeProvider split migration."""

from __future__ import annotations

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]


def read(rel: str) -> str:
    return (ROOT / rel).read_text(encoding="utf-8")


def require(pattern: str, text: str, msg: str) -> None:
    if not re.search(pattern, text, re.MULTILINE):
        raise AssertionError(msg)


def reject(pattern: str, text: str, msg: str) -> None:
    if re.search(pattern, text, re.MULTILINE):
        raise AssertionError(msg)


def check_domain_contracts() -> None:
    runtime_rs = read("crates/envr-domain/src/runtime.rs")
    require(r"pub trait RuntimeIndex: Send \+ Sync", runtime_rs, "missing RuntimeIndex trait")
    require(
        r"pub trait RuntimeInstaller: Send \+ Sync",
        runtime_rs,
        "missing RuntimeInstaller trait",
    )
    require(
        r"impl<T: RuntimeProvider \+ \?Sized> RuntimeIndex for T",
        runtime_rs,
        "missing RuntimeProvider -> RuntimeIndex adapter",
    )
    require(
        r"impl<T: RuntimeProvider \+ \?Sized> RuntimeInstaller for T",
        runtime_rs,
        "missing RuntimeProvider -> RuntimeInstaller adapter",
    )


def check_read_paths_use_index_port() -> None:
    checks: list[tuple[str, str]] = [
        ("crates/envr-cli/src/commands/current.rs", r"service\.current\("),
        ("crates/envr-cli/src/commands/list.rs", r"service\.(list_installed|current)\("),
        ("crates/envr-cli/src/commands/remote.rs", r"service\.list_remote\("),
        ("crates/envr-cli/src/commands/bundle_cmd.rs", r"service\.(resolve|current)\("),
    ]
    for rel, bad in checks:
        text = read(rel)
        reject(bad, text, f"{rel} should use RuntimeIndex via index_port")

def check_runtime_service_has_no_legacy_passthroughs() -> None:
    service_rs = read("crates/envr-core/src/runtime/service.rs")
    banned_methods = [
        r"pub fn list_installed\(",
        r"pub fn list_remote\(",
        r"pub fn list_remote_majors\(",
        r"pub fn list_remote_latest_per_major\(",
        r"pub fn resolve\(",
        r"pub fn install\(",
        r"pub fn uninstall\(",
        r"pub fn uninstall_dry_run_targets\(",
        r"pub fn current\(",
        r"pub fn set_current\(",
    ]
    for pattern in banned_methods:
        reject(
            pattern,
            service_rs,
            "legacy RuntimeService passthroughs must not be reintroduced",
        )


def main() -> int:
    try:
        check_domain_contracts()
        check_read_paths_use_index_port()
        check_runtime_service_has_no_legacy_passthroughs()
    except AssertionError as exc:
        print(f"[runtime-trait-split] FAIL: {exc}")
        return 1
    print("[runtime-trait-split] OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
