use envr_error::{EnvrError, EnvrResult};

use super::{GoProxyMode, NpmRegistryMode, PipRegistryMode, RuntimeSettings};

pub(super) fn validate_runtime_settings(runtime: &RuntimeSettings) -> EnvrResult<()> {
    validate_runtime_non_empty_strings(runtime)?;
    validate_runtime_required_custom_values(runtime)?;
    Ok(())
}

fn validate_runtime_non_empty_strings(runtime: &RuntimeSettings) -> EnvrResult<()> {
    if let Some(ref gp) = runtime.go.goproxy
        && gp.trim().is_empty()
    {
        return Err(EnvrError::Validation(
            "runtime.go.goproxy must not be whitespace-only".to_string(),
        ));
    }
    if let Some(ref v) = runtime.go.proxy_custom
        && v.trim().is_empty()
    {
        return Err(EnvrError::Validation(
            "runtime.go.proxy_custom must not be whitespace-only".to_string(),
        ));
    }
    if let Some(ref v) = runtime.node.npm_registry_url_custom
        && v.trim().is_empty()
    {
        return Err(EnvrError::Validation(
            "runtime.node.npm_registry_url_custom must not be whitespace-only".to_string(),
        ));
    }
    if let Some(ref v) = runtime.python.pip_index_url_custom
        && v.trim().is_empty()
    {
        return Err(EnvrError::Validation(
            "runtime.python.pip_index_url_custom must not be whitespace-only".to_string(),
        ));
    }
    if let Some(ref v) = runtime.go.private_patterns
        && v.trim().is_empty()
    {
        return Err(EnvrError::Validation(
            "runtime.go.private_patterns must not be whitespace-only".to_string(),
        ));
    }
    if let Some(ref dir) = runtime.bun.global_bin_dir
        && dir.trim().is_empty()
    {
        return Err(EnvrError::Validation(
            "runtime.bun.global_bin_dir must not be whitespace-only".to_string(),
        ));
    }
    Ok(())
}

fn validate_runtime_required_custom_values(runtime: &RuntimeSettings) -> EnvrResult<()> {
    if runtime.go.proxy_mode == GoProxyMode::Custom
        && runtime
            .go
            .proxy_custom
            .as_deref()
            .is_none_or(|s| s.trim().is_empty())
        && runtime
            .go
            .goproxy
            .as_deref()
            .is_none_or(|s| s.trim().is_empty())
    {
        return Err(EnvrError::Validation(
            "runtime.go.proxy_custom is required when runtime.go.proxy_mode = custom".to_string(),
        ));
    }
    if runtime.node.npm_registry_mode == NpmRegistryMode::Custom
        && runtime
            .node
            .npm_registry_url_custom
            .as_deref()
            .is_none_or(|s| s.trim().is_empty())
    {
        return Err(EnvrError::Validation(
            "runtime.node.npm_registry_url_custom is required when runtime.node.npm_registry_mode = custom"
                .to_string(),
        ));
    }
    if runtime.python.pip_registry_mode == PipRegistryMode::Custom
        && runtime
            .python
            .pip_index_url_custom
            .as_deref()
            .is_none_or(|s| s.trim().is_empty())
    {
        return Err(EnvrError::Validation(
            "runtime.python.pip_index_url_custom is required when runtime.python.pip_registry_mode = custom"
                .to_string(),
        ));
    }
    Ok(())
}
