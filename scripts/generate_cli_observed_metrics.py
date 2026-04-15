#!/usr/bin/env python3
"""
Generate observed CLI metrics from envr_cli_metrics JSONL events.

Expected input: one JSON object per line with fields like:
  phase, command, success, elapsed_ms, timestamp_ms, session_id

Usage:
  python scripts/generate_cli_observed_metrics.py --metrics-jsonl artifacts/envr-cli-metrics.jsonl
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

BOOTSTRAP_COMMANDS = {"install", "use", "init", "check"}
DAILY_COMMANDS = {"run", "exec"}
RECOVERY_COMMANDS = {"doctor", "check", "status"}
OFFLINE_COMMANDS = {"status", "current", "which", "resolve"}


def _read_jsonl(path: Path) -> list[dict]:
    rows: list[dict] = []
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line:
            continue
        try:
            item = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(item, dict):
            rows.append(item)
    return rows


def _safe_rate(successes: int, total: int) -> float:
    if total <= 0:
        return 0.0
    return successes / total


def _percentile_95(values: list[int]) -> int:
    if not values:
        return 0
    values = sorted(values)
    idx = int(round(0.95 * (len(values) - 1)))
    return values[idx]


def _session_groups(events: list[dict]) -> dict[str, list[dict]]:
    out: dict[str, list[dict]] = {}
    for idx, event in enumerate(events):
        session = event.get("session_id")
        if not isinstance(session, str) or not session:
            session = f"__synthetic_{idx}"
        out.setdefault(session, []).append(event)
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--metrics-jsonl",
        required=True,
        help="jsonl input path containing envr_cli_metrics events",
    )
    ap.add_argument(
        "--output",
        default="artifacts/cli-observed-metrics.json",
        help="output json path (repo-relative)",
    )
    args = ap.parse_args()

    input_path = Path(args.metrics_jsonl)
    if not input_path.is_absolute():
        input_path = ROOT / input_path
    rows = _read_jsonl(input_path)

    dispatch_rows = [r for r in rows if r.get("phase") == "dispatch"]
    finish_rows = [r for r in rows if r.get("phase") == "finish"]

    bootstrap_total = 0
    bootstrap_ok = 0
    for row in dispatch_rows:
        cmd = row.get("command")
        if not isinstance(cmd, str) or cmd not in BOOTSTRAP_COMMANDS:
            continue
        bootstrap_total += 1
        if row.get("success") is True:
            bootstrap_ok += 1

    daily_total = 0
    daily_ok = 0
    for row in dispatch_rows:
        cmd = row.get("command")
        if not isinstance(cmd, str) or cmd not in DAILY_COMMANDS:
            continue
        daily_total += 1
        if row.get("success") is True:
            daily_ok += 1

    # Recovery proxy from observed events:
    # a session counts recovered if any doctor dispatch exists and later a recovery command succeeds.
    recovered = 0
    recoverable = 0
    by_session = _session_groups(dispatch_rows)
    for session_rows in by_session.values():
        has_doctor = any(r.get("command") == "doctor" for r in session_rows)
        if not has_doctor:
            continue
        recoverable += 1
        ok = any(
            (isinstance(r.get("command"), str) and r.get("command") in RECOVERY_COMMANDS and r.get("success") is True)
            for r in session_rows
        )
        if ok:
            recovered += 1

    elapsed_first_success: list[int] = []
    finish_by_session = _session_groups(finish_rows)
    for session_rows in finish_by_session.values():
        ordered = sorted(
            session_rows,
            key=lambda r: int(r.get("timestamp_ms", 0)) if isinstance(r.get("timestamp_ms"), int) else 0,
        )
        if not ordered:
            continue
        first_ts = ordered[0].get("timestamp_ms")
        if not isinstance(first_ts, int):
            continue
        success_ts = None
        for item in ordered:
            if item.get("success") is True:
                ts = item.get("timestamp_ms")
                if isinstance(ts, int):
                    success_ts = ts
                    break
        if success_ts is None:
            continue
        elapsed_first_success.append(max(0, success_ts - first_ts))

    offline_elapsed: list[int] = []
    for row in dispatch_rows:
        cmd = row.get("command")
        elapsed = row.get("elapsed_ms")
        if isinstance(cmd, str) and cmd in OFFLINE_COMMANDS and isinstance(elapsed, int) and elapsed >= 0:
            offline_elapsed.append(elapsed)

    observed = {
        "bootstrap_success_rate": _safe_rate(bootstrap_ok, bootstrap_total),
        "daily_run_success_rate": _safe_rate(daily_ok, daily_total),
        "doctor_fix_recovery_rate": _safe_rate(recovered, recoverable),
        "time_to_first_success_p95_ms": _percentile_95(elapsed_first_success),
        "offline_safe_latency_p95_ms": _percentile_95(offline_elapsed),
        # Needs issue tracker context; keep sentinel so health script can override if provided.
        "extension_over_new_command_ratio": 1.0,
        "sample_size": len(rows),
    }

    out = Path(args.output)
    if not out.is_absolute():
        out = ROOT / out
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(observed, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
