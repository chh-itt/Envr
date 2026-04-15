#!/usr/bin/env python3
"""
Check governance exemption hygiene and expiry policy.

Rules:
1) Exemptions must not be expired (`due` >= today).
2) Exemptions must not exceed MAX_EXEMPT_DAYS from today.
3) Exemptions must not duplicate already-covered rows in the same map.

Usage:
  python scripts/check_cli_governance_exemptions.py
"""

from __future__ import annotations

import datetime as dt
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
INDEX_PATH = ROOT / "schemas" / "cli" / "governance-index.json"
MAX_EXEMPT_DAYS = 45


def _load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _parse_due(raw: str, section: str, command: str) -> dt.date:
    try:
        return dt.date.fromisoformat(raw)
    except ValueError as exc:
        raise ValueError(f"{section}.{command}.due must be YYYY-MM-DD, got `{raw}`") from exc


def _validate_exemptions(
    index: dict, section: str, covered_section: str, today: dt.date, bad: list[str]
) -> None:
    exemptions = index.get(section, {})
    covered = index.get(covered_section, {})
    if not isinstance(exemptions, dict):
        bad.append(f"{section} must be an object")
        return
    if not isinstance(covered, dict):
        bad.append(f"{covered_section} must be an object")
        return
    for command, meta in exemptions.items():
        if command in covered:
            bad.append(
                f"{section}.{command} is stale: command already exists in {covered_section}; remove exemption"
            )
        if not isinstance(meta, dict):
            bad.append(f"{section}.{command} must be an object")
            continue
        due_raw = meta.get("due")
        if not isinstance(due_raw, str) or not due_raw.strip():
            bad.append(f"{section}.{command}.due must be a non-empty YYYY-MM-DD string")
            continue
        due = _parse_due(due_raw.strip(), section, command)
        if due < today:
            bad.append(
                f"{section}.{command}.due expired on {due.isoformat()} (today={today.isoformat()})"
            )
            continue
        days_left = (due - today).days
        if days_left > MAX_EXEMPT_DAYS:
            bad.append(
                f"{section}.{command}.due too far in future ({days_left} days > {MAX_EXEMPT_DAYS}); keep exemptions short-lived"
            )


def _count_exemptions(index: dict, section: str) -> int:
    entries = index.get(section, {})
    if isinstance(entries, dict):
        return len(entries)
    return 0


def main() -> int:
    index = _load_json(INDEX_PATH)
    today = dt.date.today()
    bad: list[str] = []
    offline_count = _count_exemptions(index, "offline_coverage_exempt")
    capability_count = _count_exemptions(index, "capability_test_exempt")
    total_count = offline_count + capability_count
    _validate_exemptions(
        index,
        section="offline_coverage_exempt",
        covered_section="offline_coverage_rows",
        today=today,
        bad=bad,
    )
    _validate_exemptions(
        index,
        section="capability_test_exempt",
        covered_section="capability_test_rows",
        today=today,
        bad=bad,
    )
    if bad:
        print("governance exemption check failed:")
        for item in bad:
            print(f"  - {item}")
        print(
            "governance exemptions summary: "
            f"offline={offline_count}, capability={capability_count}, total={total_count}"
        )
        return 1
    print(
        "governance exemption check: ok "
        f"(offline={offline_count}, capability={capability_count}, total={total_count})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
