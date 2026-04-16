//! Contract guardrails for failure-envelope code tokens and schema stubs.

use regex::Regex;
use std::collections::BTreeSet;
use std::path::Path;

#[test]
fn every_emit_failure_envelope_code_has_failure_schema_stub() {
    let snake = Regex::new(r"^[a-z][a-z0-9_]*$").expect("snake_case regex");

    // Prefer the stable constants over source scraping. All envelope failures should route
    // through `crate::codes::err::*` so contracts are centralized.
    let codes: BTreeSet<String> = vec![
        envr_cli::codes::err::ABORTED,
        envr_cli::codes::err::ARGV_PARSE_ERROR,
        envr_cli::codes::err::VALIDATION,
        envr_cli::codes::err::CHILD_EXIT,
        envr_cli::codes::err::DIAGNOSTICS_EXPORT_FAILED,
        envr_cli::codes::err::PROJECT_CHECK_FAILED,
        envr_cli::codes::err::PROJECT_SYNC_PENDING,
        envr_cli::codes::err::PROJECT_VALIDATE_FAILED,
        envr_cli::codes::err::SHELL_EXIT,
    ]
    .into_iter()
    .map(str::to_string)
    .collect();

    for code in &codes {
        assert!(
            snake.is_match(code),
            "failure code must be snake_case: {code}"
        );
    }

    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let data_dir = manifest.join("../../schemas/cli/data");
    assert!(
        data_dir.is_dir(),
        "missing schemas dir {}",
        data_dir.display()
    );

    let mut missing = Vec::new();
    for code in &codes {
        let p = data_dir.join(format!("failure_{code}.json"));
        if !p.is_file() {
            missing.push(code.clone());
        }
    }
    assert!(
        missing.is_empty(),
        "failure codes without schemas/cli/data/failure_<code>.json:\n  {}",
        missing.join("\n  ")
    );
}
