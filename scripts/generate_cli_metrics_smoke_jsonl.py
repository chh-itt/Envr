#!/usr/bin/env python3
"""
Generate deterministic smoke envr_cli_metrics JSONL for CI observed-mode fallback.

This is used only when no real artifacts/envr-cli-metrics.jsonl is available.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def _event(
    *,
    phase: str,
    invocation_id: str,
    session_id: str,
    output_mode: str = "json",
    persona: str = "automation",
    success: bool = True,
    exit_code: int = 0,
    error_code: str = "",
    command: str | None = None,
    elapsed_ms: int | None = None,
    timestamp_ms: int | None = None,
) -> dict:
    row = {
        "phase": phase,
        "invocation_id": invocation_id,
        "session_id": session_id,
        "output_mode": output_mode,
        "persona": persona,
        "success": success,
        "exit_code": exit_code,
        "error_code": error_code,
    }
    if command is not None:
        row["command"] = command
    if elapsed_ms is not None:
        row["elapsed_ms"] = elapsed_ms
    if timestamp_ms is not None:
        row["timestamp_ms"] = timestamp_ms
    return row


def build_smoke_events() -> list[dict]:
    rows: list[dict] = []
    ts = 1_700_000_000_000
    # 10 sessions * 3 events each = 30 rows.
    commands = [
        ("install", True, 180),
        ("use", True, 120),
        ("init", True, 140),
        ("check", True, 110),
        ("run", True, 220),
        ("exec", True, 200),
        ("doctor", True, 150),
        ("status", True, 90),
        ("which", True, 80),
        ("resolve", True, 100),
    ]
    for idx, (command, success, elapsed) in enumerate(commands, start=1):
        invocation_id = f"ci-smoke-{idx:02d}"
        session_id = f"s{idx:02d}"
        rows.append(
            _event(
                phase="parse",
                invocation_id=invocation_id,
                session_id=session_id,
                success=True,
                exit_code=0,
                error_code="",
                timestamp_ms=ts,
            )
        )
        ts += 100
        rows.append(
            _event(
                phase="dispatch",
                invocation_id=invocation_id,
                session_id=session_id,
                command=command,
                success=success,
                exit_code=0 if success else 1,
                error_code="" if success else "validation",
                elapsed_ms=elapsed,
                timestamp_ms=ts,
            )
        )
        ts += 100
        rows.append(
            _event(
                phase="finish",
                invocation_id=invocation_id,
                session_id=session_id,
                success=success,
                exit_code=0 if success else 1,
                error_code="" if success else "validation",
                command=command,
                timestamp_ms=ts,
            )
        )
        ts += 100
    return rows


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--output",
        default="artifacts/envr-cli-metrics.jsonl",
        help="output jsonl path (repo-relative)",
    )
    args = ap.parse_args()

    out = Path(args.output)
    if not out.is_absolute():
        out = ROOT / out
    out.parent.mkdir(parents=True, exist_ok=True)
    rows = build_smoke_events()
    out.write_text("".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8")
    print(f"wrote {out} ({len(rows)} rows)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
