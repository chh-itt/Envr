use crate::CliExit;
use crate::cli::{GlobalArgs, ToolCmd};
use crate::output;

use envr_domain::runtime::{RUNTIME_DESCRIPTORS, runtime_descriptor};
use envr_error::{EnvrError, EnvrResult};
use serde_json::json;

pub(crate) fn run_inner(g: &GlobalArgs, cmd: ToolCmd) -> EnvrResult<CliExit> {
    match cmd {
        ToolCmd::List => list_inner(g),
        ToolCmd::Which { name } => which_inner(g, name),
        ToolCmd::Status { name } => status_inner(g, name),
    }
}

fn list_inner(g: &GlobalArgs) -> EnvrResult<CliExit> {
    let tools: Vec<_> = RUNTIME_DESCRIPTORS
        .iter()
        .map(|d| {
            let runtime = runtime_descriptor(d.kind);
            json!({
                "name": d.key,
                "label": d.label_en,
                "runtime_kind": format!("{:?}", d.kind),
                "supports_remote_latest": d.supports_remote_latest,
                "supports_path_proxy": d.supports_path_proxy,
                "host_runtime": d.host_runtime.map(|k| format!("{:?}", k)),
                "descriptor_name": runtime.key,
                "descriptor_label": runtime.label_en,
            })
        })
        .collect();
    let data = json!({ "tools": tools, "count": tools.len() });
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
    let runtime = runtime_descriptor(desc.kind);
    let data = json!({
        "name": desc.key,
        "resolved": desc.key,
        "runtime_kind": format!("{:?}", desc.kind),
        "supports_remote_latest": desc.supports_remote_latest,
        "supports_path_proxy": desc.supports_path_proxy,
        "host_runtime": desc.host_runtime.map(|k| format!("{:?}", k)),
        "descriptor_name": runtime.key,
        "descriptor_label": runtime.label_en,
    });
    Ok(output::emit_ok(g, "tool_resolved", data, || {}))
}

fn status_inner(g: &GlobalArgs, name: String) -> EnvrResult<CliExit> {
    let Some(desc) = RUNTIME_DESCRIPTORS.iter().find(|d| d.key == name) else {
        return Err(EnvrError::Validation(format!(
            "managed tool `{}` not found; run `envr tool list`",
            name
        )));
    };

    let runtime = runtime_descriptor(desc.kind);
    let data = json!({
        "name": desc.key,
        "label": desc.label_en,
        "runtime_kind": format!("{:?}", desc.kind),
        "supports_remote_latest": desc.supports_remote_latest,
        "supports_path_proxy": desc.supports_path_proxy,
        "host_runtime": desc.host_runtime.map(|k| format!("{:?}", k)),
        "descriptor_name": runtime.key,
        "descriptor_label": runtime.label_en,
    });
    Ok(output::emit_ok(g, "tool_status", data, || {
        if crate::CliUxPolicy::from_global(g).human_text_primary() {
            println!("managed tool: {}", desc.key);
            println!("label: {}", desc.label_en);
            println!("runtime kind: {:?}", desc.kind);
            println!("remote latest: {}", desc.supports_remote_latest);
            println!("path proxy: {}", desc.supports_path_proxy);
            if let Some(host) = desc.host_runtime {
                println!("host runtime: {:?}", host);
            }
            println!("descriptor name: {}", runtime.key);
        }
    }))
}
