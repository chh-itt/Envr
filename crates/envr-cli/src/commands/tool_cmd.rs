use crate::CliExit;
use crate::cli::{GlobalArgs, ToolCmd};
use crate::output;

use envr_domain::runtime::RUNTIME_DESCRIPTORS;
use envr_error::EnvrResult;
use serde_json::json;

pub(crate) fn run_inner(g: &GlobalArgs, cmd: ToolCmd) -> EnvrResult<CliExit> {
    match cmd {
        ToolCmd::List => list_inner(g),
        ToolCmd::Which { name } => which_inner(g, name),
    }
}

fn list_inner(g: &GlobalArgs) -> EnvrResult<CliExit> {
    let tools: Vec<_> = RUNTIME_DESCRIPTORS
        .iter()
        .map(|d| json!({
            "name": d.key,
            "label": d.label_en,
            "runtime_kind": format!("{:?}", d.kind),
        }))
        .collect();
    let data = json!({ "tools": tools });
    Ok(output::emit_ok(g, "tool_listed", data, || {}))
}

fn which_inner(g: &GlobalArgs, name: String) -> EnvrResult<CliExit> {
    let Some(desc) = RUNTIME_DESCRIPTORS.iter().find(|d| d.key == name) else {
        let data = json!({ "name": name, "resolved": null });
        return Ok(output::emit_failure_envelope(
            g,
            "tool_not_found",
            "managed tool not found",
            data,
            &[],
            1,
        ));
    };
    let data = json!({
        "name": desc.key,
        "resolved": desc.key,
        "runtime_kind": format!("{:?}", desc.kind),
    });
    Ok(output::emit_ok(g, "tool_resolved", data, || {}))
}
