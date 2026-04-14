//! P33: enforce a tiered policy for failure envelope `data` schemas.
//!
//! Tier policy:
//! - Tier0: strongly-typed object with non-empty `required`
//! - Tier1: object allowed, but `required` may be empty (loose / forward-compatible)
//! - Tier2: nullable (`type` includes `"null"`) to allow `data: null` where appropriate

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const OUTPUT_CONTRACT_DOC: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/cli/output-contract.md"));

fn load_schema(name: &str) -> Value {
    let src = match name {
        "failure_argv_parse_error.json" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/cli/data/failure_argv_parse_error.json"
        )),
        "failure_child_exit.json" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/cli/data/failure_child_exit.json"
        )),
        "failure_project_check_failed.json" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/cli/data/failure_project_check_failed.json"
        )),
        "failure_project_validate_failed.json" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/cli/data/failure_project_validate_failed.json"
        )),
        "failure_diagnostics_export_failed.json" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/cli/data/failure_diagnostics_export_failed.json"
        )),
        "failure_project_sync_pending.json" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/cli/data/failure_project_sync_pending.json"
        )),
        "failure_shell_exit.json" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/cli/data/failure_shell_exit.json"
        )),
        "failure_aborted.json" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/cli/data/failure_aborted.json"
        )),
        "failure_validation.json" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/cli/data/failure_validation.json"
        )),
        other => panic!("unknown schema name in test: {other}"),
    };
    serde_json::from_str(src.trim_start_matches('\u{feff}')).expect("schema json")
}

fn parse_tier_codes_from_docs() -> BTreeMap<String, Vec<String>> {
    let start = "<!-- FAILURE_DATA_TIERS_START -->";
    let end = "<!-- FAILURE_DATA_TIERS_END -->";
    let section = OUTPUT_CONTRACT_DOC
        .split_once(start)
        .and_then(|(_, rest)| rest.split_once(end).map(|(mid, _)| mid))
        .expect("failure tiers markers must exist in output-contract.md");

    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in section.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (tier, rest) = line
            .split_once(':')
            .unwrap_or_else(|| panic!("invalid tier line (missing ':'): {line:?}"));
        let tier_key = tier.split_whitespace().next().unwrap_or("").to_string();
        let mut codes: Vec<String> = vec![];
        for tok in rest.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
            if tok.is_empty() {
                continue;
            }
            if tok.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
                codes.push(tok.to_string());
            }
        }
        codes.sort();
        codes.dedup();
        out.insert(tier_key, codes);
    }
    out
}

fn schema_types(schema: &Value) -> BTreeSet<String> {
    if let Some(t) = schema.get("type") {
        return match t {
            Value::String(s) => [s.clone()].into_iter().collect(),
            Value::Array(a) => a
                .iter()
                .map(|v| v.as_str().expect("type array entries are strings").to_string())
                .collect(),
            _ => panic!("unexpected schema type: {t:?}"),
        };
    }
    if let Some(any_of) = schema.get("anyOf").and_then(|v| v.as_array()) {
        let mut out = BTreeSet::new();
        for branch in any_of {
            for t in schema_types(branch) {
                out.insert(t);
            }
        }
        return out;
    }
    panic!("schema must contain type or anyOf");
}

fn required_len(schema: &Value) -> usize {
    schema
        .get("required")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0)
}

fn has_object_branch_with_non_empty_required(schema: &Value) -> bool {
    if schema.get("type").and_then(|t| t.as_str()) == Some("object") {
        return required_len(schema) > 0;
    }
    if let Some(any_of) = schema.get("anyOf").and_then(|v| v.as_array()) {
        return any_of.iter().any(has_object_branch_with_non_empty_required);
    }
    false
}

#[test]
fn tier0_failure_data_schemas_are_strongly_typed() {
    let tiers = parse_tier_codes_from_docs();
    let tier0 = tiers.get("Tier0").expect("Tier0 list");
    for code in tier0 {
        let name = format!("failure_{code}.json");
        let schema = load_schema(&name);
        let types = schema_types(&schema);
        assert!(
            types.contains("object"),
            "tier0 schema must allow object: {code} type={types:?}"
        );
        assert!(
            has_object_branch_with_non_empty_required(&schema),
            "tier0 schema must have object branch with non-empty required: {code}"
        );
    }
}

#[test]
fn tier1_failure_data_schemas_are_loose_objects() {
    let tiers = parse_tier_codes_from_docs();
    let tier1 = tiers.get("Tier1").expect("Tier1 list");
    for code in tier1 {
        let name = format!("failure_{code}.json");
        let schema = load_schema(&name);
        let types = schema_types(&schema);
        assert!(
            types.contains("object"),
            "tier1 schema must allow object: {code} type={types:?}"
        );
        // required may be empty by policy
    }
}

#[test]
fn tier2_failure_data_schemas_allow_null() {
    let tiers = parse_tier_codes_from_docs();
    let tier2 = tiers.get("Tier2").expect("Tier2 list");
    for code in tier2 {
        let name = format!("failure_{code}.json");
        let schema = load_schema(&name);
        let types = schema_types(&schema);
        assert!(
            types.contains("null"),
            "tier2 schema must allow null: {code} type={types:?}"
        );
    }
}

