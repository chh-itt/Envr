#!/usr/bin/env python3
import unittest
import sys
from pathlib import Path

SCRIPTS_DIR = Path(__file__).resolve().parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import check_cli_contract_gate as gate
import check_cli_capability_test_coverage as capability_coverage
import check_cli_offline_coverage_alignment as offline_coverage
import check_cli_governance_index_schema as governance_index_schema
import check_cli_schema_fragments as fragments
import cli_contract_lib as lib
import generate_cli_contract_report as report
import generate_cli_contract_review_note as review_note


class ContractGateTests(unittest.TestCase):
    def test_changed_cli_schema_files_filters_expected_paths(self):
        files = [
            "schemas/cli/index.json",
            "schemas/cli/data/failure_child_exit.json",
            "schemas/other/x.json",
            "docs/cli/output-contract.md",
        ]
        self.assertEqual(
            lib.changed_cli_schema_files(files),
            ["schemas/cli/index.json", "schemas/cli/data/failure_child_exit.json"],
        )

    def test_all_breaking_reasons_collects_multiple_hits(self):
        old = {"type": "object", "properties": {"a": {"type": "string"}}}
        new = {
            "type": "object",
            "required": ["a"],
            "properties": {"a": {"type": "string", "minLength": 3}},
        }
        reasons = gate.all_breaking_reasons(old, new)
        self.assertGreaterEqual(len(reasons), 2)
        self.assertTrue(any(".required" in r for r in reasons))
        self.assertTrue(any(".minLength" in r for r in reasons))

    def test_changed_schema_ids_failure_prefix(self):
        ids = gate.changed_schema_ids(
            [
                "schemas/cli/data/failure_child_exit.json",
                "schemas/cli/data/config_set.json",
                "schemas/cli/index.json",
            ]
        )
        self.assertEqual(ids, {"child_exit", "config_set", "index"})

    def test_migration_note_mentions_all_ids(self):
        ids = {"child_exit", "config_set"}
        self.assertTrue(
            gate.migration_note_mentions_all_ids(
                "Migration note: changed child_exit and config_set", ids
            )
        )
        self.assertFalse(
            gate.migration_note_mentions_all_ids(
                "Migration note: changed child_exit", ids
            )
        )

    def test_recommend_bumps_empty_when_no_schema_changes(self):
        rec = gate.recommend_bumps([], [])
        self.assertEqual(rec["recommended_bumps"], [])
        self.assertEqual(rec["release_actions"], [])

    def test_recommend_bumps_breaking_schema_suggests_cli_json_schema_version(self):
        rec = gate.recommend_bumps(
            ["schemas/cli/data/failure_child_exit.json"],
            ["schemas/cli/data/failure_child_exit.json"],
        )
        self.assertIn("cli_json_schema_version", rec["recommended_bumps"])
        self.assertIn("add_migration_note", rec["release_actions"])

    def test_recommend_bumps_metrics_change_requires_review_action(self):
        rec = gate.recommend_bumps(["schemas/cli/metrics-event.json"], [])
        self.assertEqual(rec["recommended_bumps"], [])
        self.assertIn("review_metrics_schema_and_docs", rec["release_actions"])

    def test_porcelain_sensitive_changed_detects_expected_files(self):
        files = [
            "crates/envr-cli/src/commands/which.rs",
            "crates/envr-cli/src/commands/help_cmd.rs",
            "crates/envr-cli/src/output.rs",
        ]
        got = gate.porcelain_sensitive_changed(files)
        self.assertEqual(
            got,
            [
                "crates/envr-cli/src/commands/which.rs",
                "crates/envr-cli/src/output.rs",
            ],
        )

    def test_error_kind_map_breaking_reasons_detects_semantic_breaks(self):
        old = {
            "default": "unknown",
            "kinds": ["validation", "runtime", "unknown"],
            "map": {"child_exit": "runtime", "project_check_failed": "validation"},
        }
        new = {
            "default": "runtime",
            "kinds": ["runtime", "unknown"],
            "map": {"child_exit": "unknown"},
        }
        reasons = gate.error_kind_map_breaking_reasons(old, new)
        self.assertTrue(any("$.kinds: removed kinds" in r for r in reasons))
        self.assertTrue(any("$.default: changed" in r for r in reasons))
        self.assertTrue(any("$.map: removed mapped codes" in r for r in reasons))
        self.assertTrue(any("$.map.child_exit: changed" in r for r in reasons))

    def test_first_breaking_reason_for_path_prefers_error_kind_map_semantics(self):
        old = {
            "default": "unknown",
            "kinds": ["validation", "unknown"],
            "map": {"x": "validation"},
        }
        new = {
            "default": "unknown",
            "kinds": ["unknown"],
            "map": {"x": "validation"},
        }
        reason = gate.first_breaking_reason_for_path("schemas/cli/error-kind-map.json", old, new)
        self.assertIsNotNone(reason)
        self.assertIn("$.kinds: removed kinds", reason)

    def test_governance_index_breaking_reasons_detects_semantic_breaks(self):
        old = {
            "failure_tiers": {"tier0": ["child_exit"], "tier1": [], "tier2": []},
            "porcelain_matrix_rows": {"which": {"row_key": "which", "porcelain_expected": True}},
            "offline_coverage_rows": {"which": {"row_key": "resolve / which", "network_skip_allowed": False}},
        }
        new = {
            "failure_tiers": {"tier0": [], "tier1": [], "tier2": []},
            "porcelain_matrix_rows": {"which": {"row_key": "which", "porcelain_expected": False}},
            "offline_coverage_rows": {"which": {"row_key": "resolve / which", "network_skip_allowed": True}},
        }
        reasons = gate.governance_index_breaking_reasons(old, new)
        self.assertTrue(any("$.failure_tiers.tier0: removed codes" in r for r in reasons))
        self.assertTrue(any("$.porcelain_matrix_rows.which.porcelain_expected: changed true -> false" in r for r in reasons))
        self.assertTrue(any("$.offline_coverage_rows.which.network_skip_allowed: changed false -> true" in r for r in reasons))

    def test_first_breaking_reason_for_path_prefers_governance_index_semantics(self):
        old = {
            "failure_tiers": {"tier0": ["child_exit"], "tier1": [], "tier2": []},
            "porcelain_matrix_rows": {},
            "offline_coverage_rows": {},
        }
        new = {
            "failure_tiers": {"tier0": [], "tier1": [], "tier2": []},
            "porcelain_matrix_rows": {},
            "offline_coverage_rows": {},
        }
        reason = gate.first_breaking_reason_for_path("schemas/cli/governance-index.json", old, new)
        self.assertIsNotNone(reason)
        self.assertIn("$.failure_tiers.tier0: removed codes", reason)

    def test_analyze_governance_index_change_returns_unified_payload(self):
        old = {
            "failure_tiers": {"tier0": ["child_exit"], "tier1": [], "tier2": []},
            "porcelain_matrix_rows": {"which": {"row_key": "which", "porcelain_expected": True}},
            "offline_coverage_rows": {},
        }
        new = {
            "failure_tiers": {"tier0": [], "tier1": [], "tier2": []},
            "porcelain_matrix_rows": {"which": {"row_key": "which", "porcelain_expected": False}},
            "offline_coverage_rows": {},
        }
        analysis = gate.analyze_governance_index_change(old, new)
        self.assertTrue(analysis["breaking"])
        self.assertTrue(any("$.failure_tiers.tier0: removed codes" in r for r in analysis["reasons"]))
        self.assertEqual(analysis["summary"]["removed_tier_codes"], {"tier0": ["child_exit"]})
        self.assertIn("Migration note: governance_index changed.", analysis["migration_note_hint"])

    def test_error_kind_map_change_summary_collects_semantic_diff(self):
        old = {
            "default": "unknown",
            "kinds": ["validation", "runtime", "unknown"],
            "map": {"child_exit": "runtime", "project_check_failed": "validation"},
        }
        new = {
            "default": "runtime",
            "kinds": ["runtime", "unknown"],
            "map": {"child_exit": "unknown"},
        }
        got = report.error_kind_map_change_summary(old, new)
        self.assertEqual(got["removed_kinds"], ["validation"])
        self.assertEqual(got["default_changed"], {"from": "unknown", "to": "runtime"})
        self.assertEqual(got["removed_codes"], ["project_check_failed"])
        self.assertEqual(
            got["remapped_codes"],
            [{"code": "child_exit", "from": "runtime", "to": "unknown"}],
        )

    def test_analyze_error_kind_map_change_returns_unified_payload(self):
        old = {
            "default": "unknown",
            "kinds": ["validation", "runtime", "unknown"],
            "map": {"child_exit": "runtime"},
        }
        new = {
            "default": "runtime",
            "kinds": ["runtime", "unknown"],
            "map": {"child_exit": "unknown"},
        }
        analysis = gate.analyze_error_kind_map_change(old, new)
        self.assertTrue(analysis["breaking"])
        self.assertTrue(any("$.kinds: removed kinds" in r for r in analysis["reasons"]))
        self.assertEqual(analysis["summary"]["removed_kinds"], ["validation"])
        self.assertIn("Migration note: error_kind_map changed.", analysis["migration_note_hint"])

    def test_error_kind_map_migration_note_hint_contains_summary_bits(self):
        summary = {
            "removed_kinds": ["validation"],
            "default_changed": {"from": "unknown", "to": "runtime"},
            "removed_codes": ["project_check_failed"],
            "remapped_codes": [{"code": "child_exit", "from": "runtime", "to": "unknown"}],
        }
        hint = gate.error_kind_map_migration_note_hint(summary)
        self.assertIn("Migration note: error_kind_map changed.", hint)
        self.assertIn("removed kinds=['validation']", hint)
        self.assertIn("default 'unknown'->'runtime'", hint)
        self.assertIn("removed mapped codes=['project_check_failed']", hint)
        self.assertIn("remapped codes=[{'code': 'child_exit', 'from': 'runtime', 'to': 'unknown'}]", hint)

    def test_report_can_embed_error_kind_map_migration_note_hint(self):
        old = {
            "default": "unknown",
            "kinds": ["validation", "runtime", "unknown"],
            "map": {"child_exit": "runtime"},
        }
        new = {
            "default": "runtime",
            "kinds": ["runtime", "unknown"],
            "map": {"child_exit": "unknown"},
        }
        summary = report.error_kind_map_change_summary(old, new)
        hint = gate.error_kind_map_migration_note_hint(summary)
        self.assertIn("Migration note: error_kind_map changed.", hint)
        self.assertIn("default 'unknown'->'runtime'", hint)

    def test_render_review_note_contains_key_sections(self):
        sample = {
            "schema_changed_files": ["schemas/cli/error-kind-map.json"],
            "breaking_schema_files": ["schemas/cli/error-kind-map.json"],
            "metrics_schema_changed": False,
            "error_kind_map_changed": True,
            "error_kind_map_change_summary": {"removed_kinds": ["validation"]},
            "error_kind_map_migration_note_hint": "Migration note: error_kind_map changed.",
            "governance_index_changed": True,
            "governance_index_change_summary": {"removed_tier_codes": {"tier0": ["child_exit"]}},
            "governance_index_migration_note_hint": "Migration note: governance_index changed.",
            "release_actions": ["add_migration_note"],
            "migration_note_suggestion": "Migration note: breaking schema ids/codes changed: error-kind-map.",
        }
        note = review_note.render_review_note(sample)
        self.assertIn("## CLI Contract Review Note", note)
        self.assertIn("### Snapshot", note)
        self.assertIn("### Breaking Files", note)
        self.assertIn("### Release Actions", note)
        self.assertIn("### Migration Note Draft", note)
        self.assertIn("### Error Kind Map Change", note)
        self.assertIn("### Governance Index Change", note)
        self.assertIn("### Suggested PR Comment", note)


class SchemaFragmentTests(unittest.TestCase):
    def test_phase_a_coverage_parser_extracts_columns(self):
        md = """
## Phase A coverage map

| Command / area | JSON ok (schema where applicable) | JSON err | Porcelain |
|----------------|-----------------------------------|----------|-----------|
| `list` | `list_json_matches_schemas` | `validation_error_json_has_code` | `porcelain_list` |
"""
        got = capability_coverage.parse_phase_a_coverage(md)
        self.assertEqual(got["list"]["json_ok"], "`list_json_matches_schemas`")
        self.assertEqual(got["list"]["json_err"], "`validation_error_json_has_code`")
        self.assertEqual(got["list"]["porcelain"], "`porcelain_list`")

    def test_has_test_reference_rejects_empty_markers(self):
        self.assertFalse(capability_coverage._has_test_reference("—"))
        self.assertFalse(capability_coverage._has_test_reference("-"))
        self.assertFalse(capability_coverage._has_test_reference(" "))
        self.assertTrue(capability_coverage._has_test_reference("`list_json_matches_schemas`"))

    def test_capability_coverage_detects_missing_trace_row(self):
        report = {
            "commands": [
                {"trace_name": "list", "contract_surface": "json"},
                {"trace_name": "current", "contract_surface": "both"},
            ]
        }
        governance = {
            "capability_test_rows": {
                "list": {
                    "phase_a_row_key": "list",
                    "json_ok_required": True,
                    "porcelain_required": False,
                }
            }
        }
        phase_rows = {
            "list": {"json_ok": "`ok`", "json_err": "-", "porcelain": "-"},
            "current": {"json_ok": "`ok`", "json_err": "-", "porcelain": "-"},
        }
        errors = capability_coverage.collect_capability_test_coverage_failures(
            report, governance, phase_rows
        )
        self.assertTrue(
            any("current: missing capability_test_rows entry in governance-index" in e for e in errors)
        )

    def test_capability_coverage_allows_explicit_exemption(self):
        report = {"commands": [{"trace_name": "current", "contract_surface": "json"}]}
        governance = {
            "capability_test_rows": {},
            "capability_test_exempt": {
                "current": {
                    "reason": "pending",
                    "owner": "cli-contract",
                    "due": "2026-05-15",
                    "exit_criteria": "add tests",
                }
            },
        }
        phase_rows = {"current": {"json_ok": "`ok`", "json_err": "-", "porcelain": "-"}}
        errors = capability_coverage.collect_capability_test_coverage_failures(
            report, governance, phase_rows
        )
        self.assertEqual(errors, [])

    def test_capability_coverage_rejects_expired_exemption(self):
        report = {"commands": [{"trace_name": "current", "contract_surface": "json"}]}
        governance = {
            "capability_test_rows": {},
            "capability_test_exempt": {
                "current": {
                    "reason": "pending",
                    "owner": "cli-contract",
                    "due": "2000-01-01",
                    "exit_criteria": "add tests",
                }
            },
        }
        phase_rows = {"current": {"json_ok": "`ok`", "json_err": "-", "porcelain": "-"}}
        errors = capability_coverage.collect_capability_test_coverage_failures(
            report, governance, phase_rows
        )
        self.assertTrue(any("current: capability_test_exempt is expired" in e for e in errors))

    def test_offline_coverage_detects_missing_and_stale_mappings(self):
        report = {
            "commands": [
                {"trace_name": "list", "contract_surface": "json", "offline_safe": True},
                {"trace_name": "doctor", "contract_surface": "json", "offline_safe": True},
            ]
        }
        governance = {
            "offline_coverage_rows": {
                "list": {"row_key": "list", "network_skip_allowed": False},
                "stale_cmd": {"row_key": "old", "network_skip_allowed": False},
            }
        }
        errors = offline_coverage.collect_offline_alignment_failures(report, governance)
        self.assertTrue(any("doctor: missing offline_coverage_rows mapping" in e for e in errors))
        self.assertTrue(any("stale_cmd: offline_coverage_rows contains stale mapping" in e for e in errors))

    def test_offline_coverage_allows_explicit_exemption(self):
        report = {
            "commands": [{"trace_name": "doctor", "contract_surface": "json", "offline_safe": True}]
        }
        governance = {
            "offline_coverage_rows": {},
            "offline_coverage_exempt": {
                "doctor": {
                    "reason": "pending",
                    "owner": "cli-contract",
                    "due": "2026-05-15",
                    "exit_criteria": "add tests",
                }
            },
        }
        errors = offline_coverage.collect_offline_alignment_failures(report, governance)
        self.assertEqual(errors, [])

    def test_offline_coverage_rejects_expired_exemption(self):
        report = {
            "commands": [{"trace_name": "doctor", "contract_surface": "json", "offline_safe": True}]
        }
        governance = {
            "offline_coverage_rows": {},
            "offline_coverage_exempt": {
                "doctor": {
                    "reason": "pending",
                    "owner": "cli-contract",
                    "due": "2000-01-01",
                    "exit_criteria": "add tests",
                }
            },
        }
        errors = offline_coverage.collect_offline_alignment_failures(report, governance)
        self.assertTrue(any("doctor: offline_coverage_exempt is expired" in e for e in errors))

    def test_resolve_merge_base_falls_back_when_merge_base_missing(self):
        calls: list[list[str]] = []
        original = lib.run_git
        responses = {
            "merge-base HEAD origin/main": "",
            "merge-base --fork-point origin/main HEAD": "",
            "rev-parse HEAD~1": "abc123\n",
        }

        def fake_run_git(args: list[str], check: bool = True) -> str:
            calls.append(args)
            key = " ".join(args)
            return responses.get(key, "")

        lib.run_git = fake_run_git
        try:
            got = lib.resolve_merge_base("origin/main")
        finally:
            lib.run_git = original
        self.assertEqual(got, "abc123")
        self.assertEqual(
            calls,
            [
                ["merge-base", "HEAD", "origin/main"],
                ["merge-base", "--fork-point", "origin/main", "HEAD"],
                ["rev-parse", "HEAD~1"],
            ],
        )

    def test_governance_index_instance_schema_validation(self):
        schema = {
            "required": ["version", "failure_tiers"],
            "properties": {"version": {}, "failure_tiers": {}},
        }
        ok = {"version": 1, "failure_tiers": {"tier0": [], "tier1": [], "tier2": []}}
        self.assertEqual(
            governance_index_schema._validate_instance_schema(ok, schema),
            [],
        )
        bad = {"version": 1, "extra": True}
        errs = governance_index_schema._validate_instance_schema(bad, schema)
        self.assertTrue(any("missing required field `failure_tiers`" in e for e in errs))
        self.assertTrue(any("unexpected field `extra`" in e for e in errs))

    def test_error_kind_map_instance_schema_validation(self):
        schema = {
            "required": ["default", "kinds", "map"],
            "properties": {"default": {}, "kinds": {}, "map": {}},
        }
        ok = {
            "default": "unknown",
            "kinds": ["unknown"],
            "map": {"x": "unknown"},
        }
        self.assertEqual(
            fragments._validate_error_kind_map_instance_schema(ok, schema),
            [],
        )
        bad = {"default": "unknown", "kinds": ["unknown"], "extra": True}
        errs = fragments._validate_error_kind_map_instance_schema(bad, schema)
        self.assertTrue(any("missing required field `map`" in e for e in errs))
        self.assertTrue(any("unexpected top-level field `extra`" in e for e in errs))

    def test_error_kind_contract_validation_ok(self):
        default = "unknown"
        kinds = {
            "validation",
            "runtime",
            "io",
            "network",
            "config",
            "platform",
            "unknown",
        }
        mapping = {
            "project_check_failed": "validation",
            "child_exit": "runtime",
            "mirror": "network",
        }
        fragment = {
            "properties": {
                "kind": {
                    "enum": sorted(kinds),
                }
            }
        }
        output_rs = """
const ERROR_KIND_MAP_JSON: &str = include_str!("../../../schemas/cli/error-kind-map.json");
"""
        self.assertEqual(
            fragments._validate_error_kind_contract(fragment, output_rs, default, kinds, mapping),
            [],
        )

    def test_error_kind_contract_validation_detects_drift(self):
        default = "unknown"
        kinds = {"validation", "runtime"}
        mapping = {"child_exit": "runtime", "x": "alien"}
        fragment = {
            "properties": {
                "kind": {
                    "enum": ["validation", "runtime", "io"],
                }
            }
        }
        output_rs = """
pub fn error_kind_token(code: &str) -> &'static str {
    "validation"
}
"""
        errors = fragments._validate_error_kind_contract(fragment, output_rs, default, kinds, mapping)
        self.assertGreaterEqual(len(errors), 3)
        self.assertTrue(any("properties.kind.enum differs" in e for e in errors))
        self.assertTrue(any("default kind is not in kinds" in e for e in errors))
        self.assertTrue(any("map contains values outside kinds" in e for e in errors))
        self.assertTrue(any("must consume schemas/cli/error-kind-map.json" in e for e in errors))


if __name__ == "__main__":
    unittest.main()

