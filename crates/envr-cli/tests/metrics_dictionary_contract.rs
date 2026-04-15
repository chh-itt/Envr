//! Keep docs metrics field dictionary aligned with schema.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const METRICS_EVENT_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/metrics-event.json"
));
const OUTPUT_CONTRACT_DOC: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/cli/output-contract.md"
));

fn schema_property_names() -> BTreeSet<String> {
    let schema_src = METRICS_EVENT_SCHEMA.trim_start_matches('\u{feff}');
    let schema: Value = serde_json::from_str(schema_src).expect("schema JSON");
    let properties = schema["properties"]
        .as_object()
        .expect("metrics schema must contain properties");
    properties.keys().cloned().collect()
}

fn parse_doc_table_rows() -> BTreeMap<String, Vec<String>> {
    let start = "<!-- METRICS_FIELDS_TABLE_START -->";
    let end = "<!-- METRICS_FIELDS_TABLE_END -->";
    let section = OUTPUT_CONTRACT_DOC
        .split_once(start)
        .and_then(|(_, rest)| rest.split_once(end).map(|(mid, _)| mid))
        .expect("metrics dictionary markers must exist in output-contract.md");

    let mut rows = BTreeMap::new();
    for line in section.lines() {
        let line = line.trim();
        if !line.starts_with('|') {
            continue;
        }
        if line.contains("| field | type | required | phases |") || line.contains("|-------|") {
            continue;
        }
        let cols: Vec<String> = line
            .trim_matches('|')
            .split('|')
            .map(|c| c.trim().to_string())
            .collect();
        if cols.len() >= 5 {
            let field = cols[0].trim_matches('`').to_string();
            rows.insert(field, cols);
        }
    }
    rows
}

#[test]
fn docs_field_names_match_metrics_schema_properties() {
    let schema_names = schema_property_names();
    let doc_rows = parse_doc_table_rows();
    let doc_names: BTreeSet<String> = doc_rows.keys().cloned().collect();
    assert_eq!(
        doc_names, schema_names,
        "metrics dictionary table fields must match schemas/cli/metrics-event.json properties"
    );
}

#[test]
fn docs_table_captures_required_phase_specific_fields() {
    let doc_rows = parse_doc_table_rows();
    for field in [
        "phase",
        "output_mode",
        "persona",
        "success",
        "exit_code",
        "error_code",
    ] {
        let cols = doc_rows
            .get(field)
            .unwrap_or_else(|| panic!("missing required field row `{field}` in metrics table"));
        assert_eq!(cols[2], "yes", "field `{field}` must be marked required");
        assert_eq!(
            cols[3], "all",
            "field `{field}` must be marked as all phases"
        );
    }

    let parse_only = doc_rows.get("quiet").expect("missing `quiet` row");
    assert_eq!(parse_only[2], "yes");
    assert_eq!(parse_only[3], "parse");

    let dispatch_command = doc_rows.get("command").expect("missing `command` row");
    assert_eq!(dispatch_command[2], "yes");
    assert_eq!(dispatch_command[3], "dispatch");

    let dispatch_elapsed = doc_rows
        .get("elapsed_ms")
        .expect("missing `elapsed_ms` row");
    assert_eq!(dispatch_elapsed[2], "yes");
    assert_eq!(dispatch_elapsed[3], "dispatch");
}
