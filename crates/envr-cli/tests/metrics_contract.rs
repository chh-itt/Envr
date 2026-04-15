//! Validate phase-level `envr_cli_metrics` event shape against schema.

use serde_json::Value;

const METRICS_EVENT_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/metrics-event.json"
));

fn assert_valid(schema_src: &str, instance: &Value) {
    let schema_src = schema_src.trim_start_matches('\u{feff}');
    let schema: Value = serde_json::from_str(schema_src).expect("schema JSON");
    if let Err(e) = jsonschema::validate(&schema, instance) {
        panic!("schema validation failed: {e}");
    }
}

#[test]
fn parse_phase_event_matches_metrics_schema() {
    let event = serde_json::json!({
        "phase": "parse",
        "output_mode": "json",
        "persona": "operator",
        "quiet": false,
        "success": true,
        "exit_code": 0,
        "error_code": ""
    });
    assert_valid(METRICS_EVENT_SCHEMA, &event);
}

#[test]
fn dispatch_phase_event_matches_metrics_schema() {
    let event = serde_json::json!({
        "phase": "dispatch",
        "command": "doctor",
        "output_mode": "json",
        "persona": "automation",
        "success": false,
        "exit_code": 1,
        "error_code": "validation",
        "elapsed_ms": 12
    });
    assert_valid(METRICS_EVENT_SCHEMA, &event);
}

#[test]
fn finish_phase_event_matches_metrics_schema() {
    let event = serde_json::json!({
        "phase": "finish",
        "output_mode": "text",
        "persona": "onboarding",
        "success": false,
        "exit_code": 2,
        "error_code": "download"
    });
    assert_valid(METRICS_EVENT_SCHEMA, &event);
}
