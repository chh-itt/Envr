use envr_domain::runtime::{RuntimeKind, parse_runtime_kind};
use envr_error::{EnvrError, EnvrResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePinSpec {
    pub kind: RuntimeKind,
    /// Version or toolchain spec after `@` (e.g. `20`, `3.12`, `stable`).
    pub version: String,
}

/// Parse `KIND@SPEC` (e.g. `node@20`, `python@3.12`, `rust@stable`).
pub fn parse_runtime_pin_spec(raw: &str) -> EnvrResult<RuntimePinSpec> {
    let s = raw.trim();
    let Some((left, right)) = s.split_once('@') else {
        return Err(EnvrError::Validation(format!(
            "expected KIND@VERSION (example: node@20), got: {s:?}"
        )));
    };
    let kind = parse_runtime_kind(left.trim())?;
    let version = right.trim();
    if version.is_empty() {
        return Err(EnvrError::Validation(
            "empty version after `@` (example: node@20)".into(),
        ));
    }
    Ok(RuntimePinSpec {
        kind,
        version: version.to_string(),
    })
}

pub fn runtime_kind_toml_key(kind: RuntimeKind) -> &'static str {
    match kind {
        RuntimeKind::Node => "node",
        RuntimeKind::Python => "python",
        RuntimeKind::Java => "java",
        RuntimeKind::Go => "go",
        RuntimeKind::Rust => "rust",
        RuntimeKind::Php => "php",
        RuntimeKind::Deno => "deno",
        RuntimeKind::Bun => "bun",
        RuntimeKind::Dotnet => "dotnet",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_node_pin() {
        let p = parse_runtime_pin_spec("node@20").unwrap();
        assert_eq!(p.kind, RuntimeKind::Node);
        assert_eq!(p.version, "20");
    }

    #[test]
    fn rejects_missing_at() {
        assert!(parse_runtime_pin_spec("node20").is_err());
    }
}
