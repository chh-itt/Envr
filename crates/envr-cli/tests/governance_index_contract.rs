//! Governance index guardrails for exemption metadata quality.

use serde_json::Value;
use std::collections::BTreeSet;

const GOVERNANCE_INDEX: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/governance-index.json"
));

fn assert_exempt_entry_shape(section: &str, command: &str, value: &Value) {
    let obj = value
        .as_object()
        .unwrap_or_else(|| panic!("{section}.{command} must be a JSON object"));
    for field in ["reason", "owner", "due", "exit_criteria"] {
        let s = obj
            .get(field)
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{section}.{command}.{field} must be a non-empty string"))
            .trim();
        assert!(
            !s.is_empty(),
            "{section}.{command}.{field} must not be empty"
        );
    }
}

#[test]
fn exemption_entries_have_required_metadata_fields() {
    let v: Value = serde_json::from_str(GOVERNANCE_INDEX).expect("parse governance-index.json");
    for section in ["offline_coverage_exempt", "capability_test_exempt"] {
        let entries = v[section]
            .as_object()
            .unwrap_or_else(|| panic!("{section} must be an object"));
        for (command, item) in entries {
            assert_exempt_entry_shape(section, command, item);
        }
    }
}

#[test]
fn exemption_due_dates_are_iso_8601() {
    let v: Value = serde_json::from_str(GOVERNANCE_INDEX).expect("parse governance-index.json");
    for section in ["offline_coverage_exempt", "capability_test_exempt"] {
        let entries = v[section]
            .as_object()
            .unwrap_or_else(|| panic!("{section} must be an object"));
        for (command, item) in entries {
            let due = item
                .get("due")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("{section}.{command}.due must be a string"));
            let is_iso = due.len() == 10
                && due.chars().enumerate().all(|(idx, ch)| match idx {
                    4 | 7 => ch == '-',
                    _ => ch.is_ascii_digit(),
                });
            assert!(
                is_iso,
                "{section}.{command}.due must be ISO date (YYYY-MM-DD), got `{due}`"
            );
        }
    }
}

#[test]
fn exemptions_do_not_duplicate_covered_rows() {
    let v: Value = serde_json::from_str(GOVERNANCE_INDEX).expect("parse governance-index.json");
    let offline_rows: BTreeSet<&str> = v["offline_coverage_rows"]
        .as_object()
        .expect("offline_coverage_rows object")
        .keys()
        .map(String::as_str)
        .collect();
    let capability_rows: BTreeSet<&str> = v["capability_test_rows"]
        .as_object()
        .expect("capability_test_rows object")
        .keys()
        .map(String::as_str)
        .collect();
    let offline_exempt = v["offline_coverage_exempt"]
        .as_object()
        .expect("offline_coverage_exempt object");
    for key in offline_exempt.keys() {
        assert!(
            !offline_rows.contains(key.as_str()),
            "offline_coverage_exempt.{key} is stale because offline_coverage_rows already covers it"
        );
    }
    let capability_exempt = v["capability_test_exempt"]
        .as_object()
        .expect("capability_test_exempt object");
    for key in capability_exempt.keys() {
        assert!(
            !capability_rows.contains(key.as_str()),
            "capability_test_exempt.{key} is stale because capability_test_rows already covers it"
        );
    }
}
