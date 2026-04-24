use envr_error::{EnvrError, EnvrResult};

use super::{
    FontMode, GoProxyMode, MirrorMode, NpmRegistryMode, PipRegistryMode, RuntimeSettings, Settings,
};

pub(super) fn validate_core_settings(settings: &Settings) -> EnvrResult<()> {
    if let Some(ref root) = settings.paths.runtime_root
        && root.trim().is_empty()
    {
        return Err(EnvrError::Validation(
            "paths.runtime_root must not be whitespace-only".to_string(),
        ));
    }

    if settings.download.max_concurrent_downloads == 0 {
        return Err(EnvrError::Validation(
            "download.max_concurrent_downloads must be >= 1".to_string(),
        ));
    }

    if settings.mirror.mode == MirrorMode::Manual {
        let id_ok = settings
            .mirror
            .manual_id
            .as_deref()
            .is_some_and(|s| !s.trim().is_empty());
        if !id_ok {
            return Err(EnvrError::Validation(
                "mirror.manual_id is required when mirror.mode = manual".to_string(),
            ));
        }
    }

    if settings.appearance.font.mode == FontMode::Custom {
        let ok = settings
            .appearance
            .font
            .family
            .as_deref()
            .is_some_and(|s| !s.trim().is_empty());
        if !ok {
            return Err(EnvrError::Validation(
                "appearance.font.family is required when appearance.font.mode = custom".to_string(),
            ));
        }
    }

    if settings.gui.downloads_panel.x < 0 || settings.gui.downloads_panel.y < 0 {
        return Err(EnvrError::Validation(
            "gui.downloads_panel x/y must be >= 0".to_string(),
        ));
    }
    if let Some(xf) = settings.gui.downloads_panel.x_frac
        && (!xf.is_finite() || !(0.0..=1.0).contains(&xf))
    {
        return Err(EnvrError::Validation(
            "gui.downloads_panel x_frac must be in [0, 1]".to_string(),
        ));
    }
    if let Some(yf) = settings.gui.downloads_panel.y_frac
        && (!yf.is_finite() || !(0.0..=1.0).contains(&yf))
    {
        return Err(EnvrError::Validation(
            "gui.downloads_panel y_frac must be in [0, 1]".to_string(),
        ));
    }
    Ok(())
}

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
