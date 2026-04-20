#!/usr/bin/env python3
"""Phase 2: ensure markdown governance views stay aligned with governance-index."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GOVERNANCE_INDEX = ROOT / "schemas/cli/governance-index.json"
OUTPUT_CONTRACT = ROOT / "docs/cli/output-contract.md"
AUTOMATION_MATRIX = ROOT / "docs/cli/automation-matrix.md"


def _load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _parse_failure_tiers_from_docs(md: str) -> dict[str, set[str]]:
    start = "<!-- FAILURE_DATA_TIERS_START -->"
    end = "<!-- FAILURE_DATA_TIERS_END -->"
    if start not in md or end not in md:
        raise ValueError("docs/cli/output-contract.md: failure tier markers not found")
    section = md.split(start, 1)[1].split(end, 1)[0]
    out = {"tier0": set(), "tier1": set(), "tier2": set()}
    for raw in section.splitlines():
        line = raw.strip().lower()
        if not line or ":" not in line:
            continue
        left, right = line.split(":", 1)
        if left.startswith("tier0"):
            key = "tier0"
        elif left.startswith("tier1"):
            key = "tier1"
        elif left.startswith("tier2"):
            key = "tier2"
        else:
            continue
        for tok in right.replace("`", "").replace(",", " ").split():
            if all(c.islower() or c.isdigit() or c == "_" for c in tok):
                out[key].add(tok)
    return out


def _parse_high_traffic_porcelain_map(md: str) -> dict[str, bool]:
    start = md.find("## Matrix (high-traffic commands)")
    if start < 0:
        raise ValueError("docs/cli/automation-matrix.md: high-traffic matrix section not found")
    end = md.find("## Regression tests", start)
    if end < 0:
        end = len(md)
    body = md[start:end]
    out: dict[str, bool] = {}
    for line in body.splitlines():
        row = line.strip()
        if not row.startswith("|"):
            continue
        if row.startswith("| Command ") or row.startswith("|---------"):
            continue
        cols = [c.strip() for c in row.strip("|").split("|")]
        if len(cols) < 4:
            continue
        command = cols[0].strip().strip("`")
        porcelain = cols[3].strip().lower()
        out[command] = "yes" in porcelain
    return out


def _parse_phase_a_row_keys(md: str) -> set[str]:
    start = md.find("## Phase A coverage map")
    if start < 0:
        raise ValueError("docs/cli/automation-matrix.md: Phase A coverage map section not found")
    body = md[start:]
    out: set[str] = set()
    for line in body.splitlines():
        row = line.strip()
        if not row.startswith("|"):
            continue
        if row.startswith("| Command / area ") or row.startswith("|----------------"):
            continue
        cols = [c.strip() for c in row.strip("|").split("|")]
        if len(cols) < 3:
            continue
        out.add(cols[0].replace("`", "").strip().lower())
    return out


def main() -> int:
    bad: list[str] = []
    index = _load_json(GOVERNANCE_INDEX)

    index_tiers = index.get("failure_tiers", {})
    if not isinstance(index_tiers, dict):
        raise ValueError("schemas/cli/governance-index.json: missing failure_tiers")
    doc_tiers = _parse_failure_tiers_from_docs(OUTPUT_CONTRACT.read_text(encoding="utf-8"))
    for tier in ("tier0", "tier1", "tier2"):
        idx = set(index_tiers.get(tier, []))
        doc = doc_tiers.get(tier, set())
        if idx != doc:
            bad.append(f"failure_tiers.{tier}: index/docs mismatch (index={sorted(idx)}, docs={sorted(doc)})")

    matrix_md = AUTOMATION_MATRIX.read_text(encoding="utf-8")
    porcelain_doc_map = _parse_high_traffic_porcelain_map(matrix_md)
    porcelain_index = index.get("porcelain_matrix_rows", {})
    if not isinstance(porcelain_index, dict):
        raise ValueError("schemas/cli/governance-index.json: missing porcelain_matrix_rows object")
    for trace, spec in porcelain_index.items():
        if not isinstance(spec, dict):
            bad.append(f"porcelain_matrix_rows.{trace}: spec must be object")
            continue
        row_key = spec.get("row_key")
        expected = spec.get("porcelain_expected")
        if not isinstance(row_key, str):
            bad.append(f"porcelain_matrix_rows.{trace}: row_key must be string")
            continue
        got = porcelain_doc_map.get(row_key)
        if got is None:
            bad.append(f"porcelain_matrix_rows.{trace}: row `{row_key}` not found in docs matrix")
            continue
        if got != bool(expected):
            bad.append(
                f"porcelain_matrix_rows.{trace}: docs row `{row_key}` porcelain={got} expected={bool(expected)}"
            )

    phase_rows = _parse_phase_a_row_keys(matrix_md)
    offline_index = index.get("offline_coverage_rows", {})
    if not isinstance(offline_index, dict):
        raise ValueError("schemas/cli/governance-index.json: missing offline_coverage_rows object")
    for trace, spec in offline_index.items():
        if not isinstance(spec, dict):
            bad.append(f"offline_coverage_rows.{trace}: spec must be object")
            continue
        row_key = spec.get("row_key")
        if not isinstance(row_key, str):
            bad.append(f"offline_coverage_rows.{trace}: row_key must be string")
            continue
        if row_key not in phase_rows:
            bad.append(f"offline_coverage_rows.{trace}: row `{row_key}` not found in Phase A coverage map")

    capability_index = index.get("capability_test_rows", {})
    if not isinstance(capability_index, dict):
        raise ValueError("schemas/cli/governance-index.json: missing capability_test_rows object")
    for trace, spec in capability_index.items():
        if not isinstance(spec, dict):
            bad.append(f"capability_test_rows.{trace}: spec must be object")
            continue
        row_key = spec.get("phase_a_row_key")
        if not isinstance(row_key, str):
            bad.append(f"capability_test_rows.{trace}: phase_a_row_key must be string")
            continue
        if row_key not in phase_rows:
            bad.append(f"capability_test_rows.{trace}: row `{row_key}` not found in Phase A coverage map")

    if bad:
        print("governance index sync check failed:")
        for item in bad:
            print(f"  - {item}")
        return 1
    print("governance index sync check: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
