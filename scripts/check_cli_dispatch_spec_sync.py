#!/usr/bin/env python3
"""
Check sync between CommandSpec runtime groups and dispatch runtime routers.

Rules:
1) Every runtime CommandSpec trace_name must be routed by exactly one runtime router
   (installation / project / misc).
2) Router-derived trace_name sets must match CommandSpec runtime_group exactly.

Usage:
  python scripts/check_cli_dispatch_spec_sync.py
"""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
COMMAND_SPEC_PATH = ROOT / "crates" / "envr-cli" / "src" / "cli" / "command_spec.rs"
INSTALL_ROUTER = ROOT / "crates" / "envr-cli" / "src" / "commands" / "dispatch_runtime_installation.rs"
PROJECT_ROUTER = ROOT / "crates" / "envr-cli" / "src" / "commands" / "dispatch_runtime_project.rs"
MISC_ROUTER = ROOT / "crates" / "envr-cli" / "src" / "commands" / "dispatch_runtime_misc.rs"


def parse_spec_runtime_groups(path: Path) -> dict[str, set[str]]:
    src = path.read_text(encoding="utf-8", errors="replace")
    row_re = re.compile(
        r'CommandSpec::new\(\s*"(?P<trace>[a-z0-9_]+)"\s*,.*?,\s*(?P<runtime_required>true|false)\s*,\s*(?P<group>Some\(RuntimeHandlerGroup::[A-Za-z]+\)|None)\s*,',
        re.DOTALL,
    )
    out = {
        "Installation": set(),
        "Project": set(),
        "Misc": set(),
    }
    for m in row_re.finditer(src):
        trace = m.group("trace")
        group_raw = m.group("group")
        if group_raw == "None":
            continue
        group = group_raw.removeprefix("Some(RuntimeHandlerGroup::").removesuffix(")")
        if group not in out:
            raise ValueError(f"unknown runtime group in command_spec: {group}")
        out[group].add(trace)
    return out


def parse_command_tokens(path: Path) -> set[str]:
    src = path.read_text(encoding="utf-8", errors="replace")
    return {
        tok
        for tok in re.findall(r"Command::([A-Za-z]+)", src)
        if tok not in {"other", "Other"}
    }


def derive_installation_traces(path: Path) -> set[str]:
    tokens = parse_command_tokens(path)
    mapping = {
        "Install": "install",
        "Use": "use",
        "List": "list",
        "Current": "current",
        "Uninstall": "uninstall",
    }
    return {mapping[t] for t in tokens if t in mapping}


def derive_project_traces(path: Path) -> set[str]:
    tokens = parse_command_tokens(path)
    out: set[str] = set()
    if "Prune" in tokens:
        out.add("prune")
    if "Project" in tokens:
        out.update({"project_add", "project_sync", "project_validate"})
    return out


def derive_misc_traces(path: Path) -> set[str]:
    tokens = parse_command_tokens(path)
    mapping = {
        "Doctor": "doctor",
        "Remote": "remote",
        "Diagnostics": "diagnostics_export",
    }
    return {mapping[t] for t in tokens if t in mapping}


def check_group(name: str, expected: set[str], actual: set[str], bad: list[str]) -> None:
    missing = sorted(expected - actual)
    extra = sorted(actual - expected)
    if missing:
        bad.append(f"{name}: missing router coverage for {missing}")
    if extra:
        bad.append(f"{name}: router has traces not in CommandSpec group: {extra}")


def main() -> int:
    spec_groups = parse_spec_runtime_groups(COMMAND_SPEC_PATH)
    router_groups = {
        "Installation": derive_installation_traces(INSTALL_ROUTER),
        "Project": derive_project_traces(PROJECT_ROUTER),
        "Misc": derive_misc_traces(MISC_ROUTER),
    }
    bad: list[str] = []
    for group in ("Installation", "Project", "Misc"):
        check_group(group, spec_groups[group], router_groups[group], bad)

    total_spec_runtime = set().union(*spec_groups.values())
    total_router_runtime = set().union(*router_groups.values())
    if total_spec_runtime != total_router_runtime:
        bad.append(
            "runtime union mismatch between CommandSpec and runtime routers "
            f"(spec={sorted(total_spec_runtime)}, routers={sorted(total_router_runtime)})"
        )

    if bad:
        print("dispatch/spec sync check failed:")
        for item in bad:
            print(f"  - {item}")
        return 1
    print("dispatch/spec sync check: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
