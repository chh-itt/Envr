#!/usr/bin/env python3
"""
Generate v1 health report artifact from existing CLI governance artifacts.

Usage:
  python scripts/generate_cli_v1_health_report.py
  python scripts/generate_cli_v1_health_report.py --output artifacts/cli-v1-health.json
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_REPORT_PATH = ROOT / "artifacts/cli-contract-report.json"
CAPABILITIES_REPORT_PATH = ROOT / "artifacts/cli-capabilities-report.json"
GOVERNANCE_INDEX_PATH = ROOT / "schemas/cli/governance-index.json"
COMMAND_SPEC_PATH = ROOT / "crates/envr-cli/src/cli/command_spec.rs"
OBSERVED_METRICS_PATH = ROOT / "artifacts/cli-observed-metrics.json"


def _load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _count_long_lived_exemptions(index: dict) -> int:
    today = dt.date.today()
    count = 0
    for section in ("offline_coverage_exempt", "capability_test_exempt"):
        entries = index.get(section, {})
        if not isinstance(entries, dict):
            continue
        for meta in entries.values():
            if not isinstance(meta, dict):
                continue
            due = meta.get("due")
            if not isinstance(due, str):
                continue
            try:
                due_date = dt.date.fromisoformat(due)
            except ValueError:
                # Bad due format should be treated as hygiene debt.
                count += 1
                continue
            if due_date < today:
                count += 1
    return count


def _count_top_level_commands(command_spec_src: str) -> int:
    """
    Count unique top-level command names from help_path entries in COMMAND_SPEC_REGISTRY.
    Example fragment: &["cache", "index", "sync"] -> top-level "cache"
    """
    top_level: set[str] = set()
    in_registry = False
    for line in command_spec_src.splitlines():
        if "const COMMAND_SPEC_REGISTRY" in line:
            in_registry = True
            continue
        if in_registry and line.strip() == "];":
            break
        if not in_registry:
            continue
        m = re.search(r'&\[\s*"([^"]+)"', line)
        if m:
            top_level.add(m.group(1))
    return len(top_level)


def _required_metric_names() -> list[str]:
    return [
        "bootstrap_success_rate",
        "daily_run_success_rate",
        "doctor_fix_recovery_rate",
        "time_to_first_success_p95_ms",
        "offline_safe_latency_p95_ms",
        "extension_over_new_command_ratio",
    ]


def _is_valid_observed_metrics(metrics: dict, min_sample_size: int) -> tuple[bool, list[str]]:
    bad: list[str] = []
    for key in _required_metric_names():
        if key not in metrics:
            bad.append(f"missing `{key}`")
    for key in (
        "bootstrap_success_rate",
        "daily_run_success_rate",
        "doctor_fix_recovery_rate",
        "extension_over_new_command_ratio",
    ):
        value = metrics.get(key)
        if not isinstance(value, (int, float)) or not (0.0 <= float(value) <= 1.0):
            bad.append(f"`{key}` out of range [0,1]: {value!r}")
    for key in ("time_to_first_success_p95_ms", "offline_safe_latency_p95_ms"):
        value = metrics.get(key)
        if not isinstance(value, int) or value < 0:
            bad.append(f"`{key}` must be non-negative integer: {value!r}")

    sample_size = metrics.get("sample_size")
    if not isinstance(sample_size, int) or sample_size < min_sample_size:
        bad.append(
            f"`sample_size` too small or invalid ({sample_size!r} < {min_sample_size})"
        )
    return (len(bad) == 0, bad)


def _threshold_violations(metrics: dict) -> list[str]:
    out: list[str] = []
    if metrics["bootstrap_success_rate"] < 0.95:
        out.append("bootstrap_success_rate < 0.95")
    if metrics["daily_run_success_rate"] < 0.99:
        out.append("daily_run_success_rate < 0.99")
    if metrics["doctor_fix_recovery_rate"] < 0.85:
        out.append("doctor_fix_recovery_rate < 0.85")
    if metrics["time_to_first_success_p95_ms"] > 600000:
        out.append("time_to_first_success_p95_ms > 600000")
    if metrics["offline_safe_latency_p95_ms"] >= 300:
        out.append("offline_safe_latency_p95_ms >= 300")
    if metrics["extension_over_new_command_ratio"] < 0.70:
        out.append("extension_over_new_command_ratio < 0.70")
    return out


def _capability_keys_with_exemptions(governance_index: dict) -> set[str]:
    rows = governance_index.get("capability_test_rows", {})
    exempt = governance_index.get("capability_test_exempt", {})
    keys: set[str] = set()
    if isinstance(rows, dict):
        keys.update(rows.keys())
    if isinstance(exempt, dict):
        keys.update(exempt.keys())
    return keys


def _command_trace_index(capabilities_report: dict) -> dict[str, dict]:
    out: dict[str, dict] = {}
    commands = capabilities_report.get("commands", [])
    if not isinstance(commands, list):
        return out
    for row in commands:
        if not isinstance(row, dict):
            continue
        trace_name = row.get("trace_name")
        if isinstance(trace_name, str):
            out[trace_name] = row
    return out


def _coverage_ratio(required: list[str], available: set[str]) -> float:
    if not required:
        return 1.0
    hits = sum(1 for item in required if item in available)
    return hits / len(required)


def _proxy_time_to_first_success_ms(bootstrap_rate: float) -> int:
    # Conservative proxy without session-level telemetry.
    if bootstrap_rate >= 0.95:
        return 420000
    if bootstrap_rate >= 0.75:
        return 600000
    return 900000


def _proxy_offline_latency_ms(command_index: dict[str, dict]) -> int:
    critical = ["status", "current", "which", "resolve"]
    misses = 0
    for name in critical:
        row = command_index.get(name)
        if not row or row.get("offline_safe") is not True:
            misses += 1
    return 180 + 80 * misses


def _proxy_extension_ratio(
    top_level_count: int,
    baseline: int | None,
    contract_report: dict,
) -> float:
    if baseline is not None and baseline > 0:
        added = max(0, top_level_count - baseline)
        if added == 0:
            return 1.0
        return max(0.0, 1.0 - (added / top_level_count))
    if contract_report.get("capabilities_registry_changed") is True:
        return 0.7
    return 1.0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--contract-report",
        default=str(CONTRACT_REPORT_PATH.relative_to(ROOT)),
        help="path to artifacts/cli-contract-report.json",
    )
    ap.add_argument(
        "--capabilities-report",
        default=str(CAPABILITIES_REPORT_PATH.relative_to(ROOT)),
        help="path to artifacts/cli-capabilities-report.json",
    )
    ap.add_argument(
        "--governance-index",
        default=str(GOVERNANCE_INDEX_PATH.relative_to(ROOT)),
        help="path to schemas/cli/governance-index.json",
    )
    ap.add_argument(
        "--command-spec",
        default=str(COMMAND_SPEC_PATH.relative_to(ROOT)),
        help="path to command spec source",
    )
    ap.add_argument(
        "--output",
        default="artifacts/cli-v1-health.json",
        help="output json path (repo-relative)",
    )
    ap.add_argument(
        "--observed-metrics-json",
        default=None,
        help="optional json file containing observed metrics; overrides proxy values",
    )
    ap.add_argument(
        "--window-from",
        default=dt.date.today().isoformat(),
        help="window start date YYYY-MM-DD",
    )
    ap.add_argument(
        "--window-to",
        default=dt.date.today().isoformat(),
        help="window end date YYYY-MM-DD",
    )
    ap.add_argument(
        "--sample-size",
        type=int,
        default=None,
        help="optional sample size for the reporting window",
    )
    ap.add_argument(
        "--min-observed-sample-size",
        type=int,
        default=20,
        help="minimum observed sample size to trust observed metric mode",
    )
    ap.add_argument(
        "--governance-gate-passed",
        choices=("true", "false"),
        default="true",
        help="whether governance checks passed before this report generation",
    )
    ap.add_argument("--bootstrap-success-rate", type=float, default=None)
    ap.add_argument("--daily-run-success-rate", type=float, default=None)
    ap.add_argument("--doctor-fix-recovery-rate", type=float, default=None)
    ap.add_argument("--time-to-first-success-p95-ms", type=int, default=None)
    ap.add_argument("--offline-safe-latency-p95-ms", type=int, default=None)
    ap.add_argument("--extension-over-new-command-ratio", type=float, default=None)
    ap.add_argument("--top-level-command-baseline", type=int, default=None)
    args = ap.parse_args()

    contract_report_path = ROOT / args.contract_report
    capabilities_report_path = ROOT / args.capabilities_report
    governance_index_path = ROOT / args.governance_index
    command_spec_path = ROOT / args.command_spec
    if args.observed_metrics_json:
        observed_metrics_path = Path(args.observed_metrics_json)
        if not observed_metrics_path.is_absolute():
            observed_metrics_path = ROOT / observed_metrics_path
    else:
        observed_metrics_path = OBSERVED_METRICS_PATH

    contract_report = _load_json(contract_report_path)
    capabilities_report = _load_json(capabilities_report_path)
    governance_index = _load_json(governance_index_path)
    command_spec_src = command_spec_path.read_text(encoding="utf-8")
    observed_metrics: dict = {}
    observed_metrics_valid = False
    observed_metrics_rejected_reasons: list[str] = []
    observed_metrics_source: str = ""
    if observed_metrics_path.is_file():
        loaded = _load_json(observed_metrics_path)
        if isinstance(loaded, dict):
            observed_metrics_valid, observed_metrics_rejected_reasons = _is_valid_observed_metrics(
                loaded, args.min_observed_sample_size
            )
            if observed_metrics_valid:
                observed_metrics = loaded
                src = loaded.get("observed_source")
                if isinstance(src, str):
                    observed_metrics_source = src

    breaking_contract_changes = len(contract_report.get("breaking_schema_files", []))
    governance_gate_passed = args.governance_gate_passed == "true"
    long_lived_exemptions = _count_long_lived_exemptions(governance_index)
    top_level_command_count = _count_top_level_commands(command_spec_src)

    command_index = _command_trace_index(capabilities_report)
    capability_keys = _capability_keys_with_exemptions(governance_index)
    covered_commands = set(command_index.keys()).intersection(capability_keys)

    proxy_bootstrap_success_rate = _coverage_ratio(["install", "use", "init", "check"], covered_commands)
    proxy_daily_run_success_rate = _coverage_ratio(["run", "exec"], covered_commands)
    proxy_doctor_fix_recovery_rate = _coverage_ratio(["doctor", "check", "status"], covered_commands)
    proxy_time_to_first_success_p95_ms = _proxy_time_to_first_success_ms(proxy_bootstrap_success_rate)
    proxy_offline_safe_latency_p95_ms = _proxy_offline_latency_ms(command_index)
    proxy_extension_over_new_command_ratio = _proxy_extension_ratio(
        top_level_command_count, args.top_level_command_baseline, contract_report
    )

    metrics = {
        "bootstrap_success_rate": (
            args.bootstrap_success_rate
            if args.bootstrap_success_rate is not None
            else observed_metrics.get("bootstrap_success_rate", proxy_bootstrap_success_rate)
        ),
        "daily_run_success_rate": (
            args.daily_run_success_rate
            if args.daily_run_success_rate is not None
            else observed_metrics.get("daily_run_success_rate", proxy_daily_run_success_rate)
        ),
        "doctor_fix_recovery_rate": (
            args.doctor_fix_recovery_rate
            if args.doctor_fix_recovery_rate is not None
            else observed_metrics.get("doctor_fix_recovery_rate", proxy_doctor_fix_recovery_rate)
        ),
        "time_to_first_success_p95_ms": (
            args.time_to_first_success_p95_ms
            if args.time_to_first_success_p95_ms is not None
            else observed_metrics.get("time_to_first_success_p95_ms", proxy_time_to_first_success_p95_ms)
        ),
        "offline_safe_latency_p95_ms": (
            args.offline_safe_latency_p95_ms
            if args.offline_safe_latency_p95_ms is not None
            else observed_metrics.get("offline_safe_latency_p95_ms", proxy_offline_safe_latency_p95_ms)
        ),
        "breaking_contract_changes": breaking_contract_changes,
        "governance_gate_passed": governance_gate_passed,
        "long_lived_exemptions": long_lived_exemptions,
        "top_level_command_count": top_level_command_count,
        "top_level_command_baseline": args.top_level_command_baseline,
        "extension_over_new_command_ratio": (
            args.extension_over_new_command_ratio
            if args.extension_over_new_command_ratio is not None
            else observed_metrics.get("extension_over_new_command_ratio", proxy_extension_over_new_command_ratio)
        ),
    }

    blocking_reasons: list[str] = []
    if breaking_contract_changes > 0:
        blocking_reasons.append("breaking_contract_changes > 0")
    if not governance_gate_passed:
        blocking_reasons.append("governance_gate_passed == false")
    if long_lived_exemptions > 0:
        blocking_reasons.append("long_lived_exemptions > 0")
    if args.top_level_command_baseline is not None and top_level_command_count != args.top_level_command_baseline:
        blocking_reasons.append(
            f"top_level_command_count changed ({top_level_command_count} != {args.top_level_command_baseline})"
        )

    missing_required_metrics = [name for name in _required_metric_names() if metrics[name] is None]
    threshold_violations = _threshold_violations(metrics) if not missing_required_metrics else []
    hard_guard_passed = len(blocking_reasons) == 0
    dod_passed = hard_guard_passed and len(missing_required_metrics) == 0 and len(threshold_violations) == 0

    observed_fields = set(observed_metrics.keys()).intersection(
        {
            "bootstrap_success_rate",
            "daily_run_success_rate",
            "doctor_fix_recovery_rate",
            "time_to_first_success_p95_ms",
            "offline_safe_latency_p95_ms",
            "extension_over_new_command_ratio",
        }
    )
    explicit_fields = {
        "bootstrap_success_rate": args.bootstrap_success_rate,
        "daily_run_success_rate": args.daily_run_success_rate,
        "doctor_fix_recovery_rate": args.doctor_fix_recovery_rate,
        "time_to_first_success_p95_ms": args.time_to_first_success_p95_ms,
        "offline_safe_latency_p95_ms": args.offline_safe_latency_p95_ms,
        "extension_over_new_command_ratio": args.extension_over_new_command_ratio,
    }
    all_explicit = all(v is not None for v in explicit_fields.values())
    all_observed = len(observed_fields) == len(explicit_fields)

    window_sample_size = args.sample_size
    if window_sample_size is None:
        observed_sample_size = observed_metrics.get("sample_size")
        if isinstance(observed_sample_size, int) and observed_sample_size >= 0:
            window_sample_size = observed_sample_size

    report = {
        "report_version": 1,
        "generated_at": dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat(),
        "window": {
            "from": args.window_from,
            "to": args.window_to,
            "sample_size": window_sample_size,
        },
        "summary": {
            "hard_guard_passed": hard_guard_passed,
            "dod_passed": dod_passed,
            "blocking_reasons": blocking_reasons,
            "missing_required_metrics": missing_required_metrics,
            "threshold_violations": threshold_violations,
            "metric_mode": (
                "observed"
                if not missing_required_metrics and (all_explicit or all_observed)
                else "proxy"
            ),
            "observed_metrics_valid": observed_metrics_valid,
            "observed_metrics_rejected_reasons": observed_metrics_rejected_reasons,
            "observed_metrics_source": observed_metrics_source,
        },
        "metrics": metrics,
        "thresholds": {
            "bootstrap_success_rate": ">=0.95",
            "daily_run_success_rate": ">=0.99",
            "doctor_fix_recovery_rate": ">=0.85",
            "time_to_first_success_p95_ms": "<=600000",
            "offline_safe_latency_p95_ms": "<300",
            "breaking_contract_changes": "==0",
            "governance_gate_passed": "==true",
            "long_lived_exemptions": "==0",
            "extension_over_new_command_ratio": ">=0.70",
            "top_level_command_freeze": "equals baseline when baseline is provided",
        },
        "sources": {
            "contract_report": args.contract_report,
            "capabilities_report": args.capabilities_report,
            "governance_index": args.governance_index,
            "command_spec": args.command_spec,
            "observed_metrics": (
                str((args.observed_metrics_json or str(OBSERVED_METRICS_PATH.relative_to(ROOT))))
                if observed_metrics
                else ""
            ),
        },
    }

    out = Path(args.output)
    if not out.is_absolute():
        out = ROOT / out
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
