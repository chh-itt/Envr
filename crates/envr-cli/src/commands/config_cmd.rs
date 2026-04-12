//! `envr config` — path and show for `settings.toml`.

use crate::cli::{ConfigValueType, GlobalArgs, OutputFormat};
use crate::output;

use envr_config::settings::{Settings, validate_settings_file};
use envr_config::settings_toml_schema_template_zh;
use envr_error::EnvrError;
use envr_platform::paths::current_platform_paths;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::Command;

pub fn run(g: &GlobalArgs, sub: crate::cli::ConfigCmd) -> i32 {
    let paths = match current_platform_paths() {
        Ok(p) => p,
        Err(e) => return crate::commands::common::print_envr_error(g, e),
    };
    let settings_path = envr_config::settings::settings_path_from_platform(&paths);

    let migrated = match ensure_migrated_settings_file(&settings_path) {
        Ok(v) => v,
        Err(e) => return crate::commands::common::print_envr_error(g, e),
    };

    match sub {
        crate::cli::ConfigCmd::Schema => {
            let tpl = settings_toml_schema_template_zh();
            let data = serde_json::json!({
                "path": settings_path.to_string_lossy(),
                "template": tpl,
                "migrated": migrated,
            });
            output::emit_ok(g, "config_schema", data, || {
                print!("{tpl}");
            })
        }
        crate::cli::ConfigCmd::Validate => match validate_settings_file(&settings_path) {
            Ok(()) => {
                let data = serde_json::json!({
                    "path": settings_path.to_string_lossy(),
                    "valid": true,
                    "migrated": migrated,
                });
                output::emit_ok(g, "config_validate_ok", data, || {
                    if !g.quiet {
                        println!(
                            "{}",
                            envr_core::i18n::tr_key(
                                "cli.config.validate_ok",
                                "settings.toml 校验通过",
                                "`settings.toml` is valid",
                            )
                        );
                        println!("{}", settings_path.display());
                    }
                })
            }
            Err(e) => crate::commands::common::print_envr_error(g, e),
        },
        crate::cli::ConfigCmd::Edit => {
            if let Err(e) = edit_settings_loop(g, &settings_path, migrated) {
                return crate::commands::common::print_envr_error(g, e);
            }
            let data = serde_json::json!({
                "path": settings_path.to_string_lossy(),
                "migrated": migrated,
            });
            output::emit_ok(g, "config_edit_ok", data, || {})
        }
        crate::cli::ConfigCmd::Path => {
            let data = serde_json::json!({ "path": settings_path.to_string_lossy() });
            output::emit_ok(g, "config_path", data, || {
                println!("{}", settings_path.display());
            })
        }
        crate::cli::ConfigCmd::Keys => {
            let keys = config_writable_keys();
            let data = serde_json::json!({
                "path": settings_path.to_string_lossy(),
                "keys": keys,
                "migrated": migrated,
            });
            output::emit_ok(g, "config_keys", data, || {
                for k in keys {
                    println!("{k}");
                }
            })
        }
        crate::cli::ConfigCmd::Get { key } => match Settings::load_or_default_from(&settings_path) {
            Ok(st) => {
                let v = match serde_json::to_value(&st) {
                    Ok(v) => v,
                    Err(e) => {
                        return crate::commands::common::print_envr_error(
                            g,
                            EnvrError::Runtime(format!("json encode settings: {e}")),
                        );
                    }
                };
                let got = get_json_dotted(&v, &key);
                let data = serde_json::json!({
                    "path": settings_path.to_string_lossy(),
                    "key": key,
                    "value": got.cloned().unwrap_or(serde_json::Value::Null),
                    "migrated": migrated,
                });
                output::emit_ok(g, "config_get", data, || {
                    if let Some(val) = got {
                        if val.is_string() {
                            println!("{}", val.as_str().unwrap_or(""));
                        } else {
                            println!("{val}");
                        }
                    }
                })
            }
            Err(e) => crate::commands::common::print_envr_error(g, e),
        },
        crate::cli::ConfigCmd::Set {
            key,
            value,
            value_type,
        } => {
            let mut st = match Settings::load_or_default_from(&settings_path) {
                Ok(s) => s,
                Err(e) => return crate::commands::common::print_envr_error(g, e),
            };
            let mut as_json = match serde_json::to_value(&st) {
                Ok(v) => v,
                Err(e) => {
                    return crate::commands::common::print_envr_error(
                        g,
                        EnvrError::Runtime(format!("json encode settings: {e}")),
                    );
                }
            };
            if get_json_dotted(&as_json, &key).is_none() {
                let hint = suggest_key_hint(&key);
                let msg = if let Some(h) = hint {
                    format!("unknown config key `{key}`; try `{h}`")
                } else {
                    format!("unknown config key `{key}`")
                };
                return crate::commands::common::print_envr_error(g, EnvrError::Validation(msg));
            }
            let parsed = match parse_user_value(&value, value_type) {
                Ok(v) => v,
                Err(e) => return crate::commands::common::print_envr_error(g, e),
            };
            if let Err(e) = set_json_dotted(&mut as_json, &key, parsed.clone()) {
                return crate::commands::common::print_envr_error(g, EnvrError::Validation(e));
            }
            st = match serde_json::from_value(as_json) {
                Ok(v) => v,
                Err(e) => {
                    return crate::commands::common::print_envr_error(
                        g,
                        EnvrError::Validation(format!("invalid value for `{key}`: {e}")),
                    );
                }
            };
            if let Err(e) = st.validate() {
                return crate::commands::common::print_envr_error(g, e);
            }
            if let Err(e) = st.save_to(&settings_path) {
                return crate::commands::common::print_envr_error(g, e);
            }
            let data = serde_json::json!({
                "path": settings_path.to_string_lossy(),
                "key": key,
                "value": parsed,
                "migrated": migrated,
            });
            output::emit_ok(g, "config_set", data, || {
                println!("{}", settings_path.display());
            })
        }
        crate::cli::ConfigCmd::Show => match Settings::load_or_default_from(&settings_path) {
            Ok(st) => {
                let pretty = match toml::to_string_pretty(&st) {
                    Ok(s) => s,
                    Err(e) => {
                        return crate::commands::common::print_envr_error(
                            g,
                            EnvrError::Runtime(format!("toml encode: {e}")),
                        );
                    }
                };
                let data = serde_json::json!({
                    "path": settings_path.to_string_lossy(),
                    "settings": serde_json::to_value(&st).unwrap_or(serde_json::Value::Null),
                    "migrated": migrated,
                });
                output::emit_ok(g, "config_show", data, || {
                    println!("{}", settings_path.display());
                    println!();
                    print!("{pretty}");
                })
            }
            Err(e) => crate::commands::common::print_envr_error(g, e),
        },
    }
}

fn editor_invocation() -> Result<String, EnvrError> {
    for key in ["VISUAL", "EDITOR", "GIT_EDITOR"] {
        if let Ok(v) = std::env::var(key) {
            let t = v.trim();
            if !t.is_empty() {
                return Ok(t.to_string());
            }
        }
    }
    #[cfg(windows)]
    {
        Ok("notepad".to_string())
    }
    #[cfg(not(windows))]
    {
        Ok("vi".to_string())
    }
}

fn run_editor(editor: &str, path: &Path) -> Result<std::process::ExitStatus, EnvrError> {
    let ed = editor.trim();
    if ed.is_empty() {
        return Err(EnvrError::Validation("empty editor command".into()));
    }
    #[cfg(windows)]
    {
        let status = Command::new("cmd")
            .arg("/C")
            .arg(format!("{} \"{}\"", ed, path.display()))
            .status()
            .map_err(EnvrError::from)?;
        return Ok(status);
    }
    #[cfg(not(windows))]
    {
        let status = Command::new("sh")
            .arg("-c")
            .arg(format!("{ed} \"$1\""))
            .arg("_")
            .arg(path)
            .status()
            .map_err(EnvrError::from)?;
        Ok(status)
    }
}

fn edit_settings_loop(g: &GlobalArgs, path: &Path, _migrated: bool) -> Result<(), EnvrError> {
    if !path.exists() {
        let st = Settings::default();
        st.validate()?;
        st.save_to(path)?;
    }
    let _ = ensure_migrated_settings_file(path)?;

    let interactive = matches!(
        g.output_format.unwrap_or(OutputFormat::Text),
        OutputFormat::Text
    ) && io::stdin().is_terminal();

    loop {
        let editor = editor_invocation()?;
        let status = run_editor(&editor, path)?;
        if !status.success() {
            return Err(EnvrError::Runtime(format!(
                "editor exited with status {:?}",
                status.code()
            )));
        }
        match Settings::load_from(path) {
            Ok(_) => return Ok(()),
            Err(e) => {
                if !interactive {
                    return Err(e);
                }
                eprintln!("{e}");
                let prompt = envr_core::i18n::tr_key(
                    "cli.config.edit_reopen_prompt",
                    "配置无效。重新打开编辑器？[y/N] ",
                    "Settings invalid. Re-open editor? [y/N] ",
                );
                print!("{prompt}");
                io::stdout().flush().map_err(EnvrError::from)?;
                let mut line = String::new();
                io::stdin().read_line(&mut line).map_err(EnvrError::from)?;
                let y = matches!(
                    line.trim().to_ascii_lowercase().as_str(),
                    "y" | "yes"
                );
                if !y {
                    return Err(e);
                }
            }
        }
    }
}

fn parse_user_value(
    raw: &str,
    value_type: Option<ConfigValueType>,
) -> Result<serde_json::Value, EnvrError> {
    let trimmed = raw.trim();
    if let Some(t) = value_type {
        let parsed = match t {
            ConfigValueType::String => serde_json::Value::String(raw.to_string()),
            ConfigValueType::Bool => {
                let b = match trimmed.to_ascii_lowercase().as_str() {
                    "true" | "1" | "yes" | "on" => true,
                    "false" | "0" | "no" | "off" => false,
                    _ => {
                        return Err(EnvrError::Validation(format!(
                            "invalid bool value `{raw}` (expected true/false)"
                        )));
                    }
                };
                serde_json::Value::Bool(b)
            }
            ConfigValueType::Int => {
                let i = trimmed.parse::<i64>().map_err(|_| {
                    EnvrError::Validation(format!("invalid int value `{raw}`"))
                })?;
                serde_json::Value::Number(i.into())
            }
            ConfigValueType::Float => {
                let f = trimmed.parse::<f64>().map_err(|_| {
                    EnvrError::Validation(format!("invalid float value `{raw}`"))
                })?;
                let n = serde_json::Number::from_f64(f).ok_or_else(|| {
                    EnvrError::Validation(format!("invalid float value `{raw}`"))
                })?;
                serde_json::Value::Number(n)
            }
            ConfigValueType::Json => serde_json::from_str::<serde_json::Value>(trimmed).map_err(
                |_| EnvrError::Validation(format!("invalid json value `{raw}`")),
            )?,
        };
        return Ok(parsed);
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return Ok(v);
    }
    if trimmed.eq_ignore_ascii_case("true") {
        return Ok(serde_json::Value::Bool(true));
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return Ok(serde_json::Value::Bool(false));
    }
    if let Ok(i) = trimmed.parse::<i64>() {
        return Ok(serde_json::Value::Number(i.into()));
    }
    if let Ok(f) = trimmed.parse::<f64>()
        && let Some(n) = serde_json::Number::from_f64(f)
    {
        return Ok(serde_json::Value::Number(n));
    }
    Ok(serde_json::Value::String(raw.to_string()))
}

fn get_json_dotted<'a>(root: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    let mut cur = root;
    for seg in key.split('.').filter(|s| !s.trim().is_empty()) {
        cur = cur.get(seg)?;
    }
    Some(cur)
}

fn set_json_dotted(
    root: &mut serde_json::Value,
    key: &str,
    value: serde_json::Value,
) -> Result<(), String> {
    let parts: Vec<&str> = key.split('.').filter(|s| !s.trim().is_empty()).collect();
    if parts.is_empty() {
        return Err("empty key".to_string());
    }
    let mut cur = root;
    for seg in &parts[..parts.len() - 1] {
        let obj = cur
            .as_object_mut()
            .ok_or_else(|| format!("`{seg}` parent is not an object"))?;
        cur = obj
            .entry((*seg).to_string())
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    }
    let Some(last) = parts.last() else {
        return Err("invalid key".to_string());
    };
    let obj = cur
        .as_object_mut()
        .ok_or_else(|| "target parent is not an object".to_string())?;
    obj.insert((*last).to_string(), value);
    Ok(())
}

fn backup_path(path: &Path) -> std::path::PathBuf {
    let mut out = path.to_path_buf();
    let ext = out
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| format!("{s}.bak"))
        .unwrap_or_else(|| "bak".to_string());
    out.set_extension(ext);
    out
}

fn ensure_migrated_settings_file(path: &Path) -> Result<bool, EnvrError> {
    if !path.exists() {
        return Ok(false);
    }
    let raw = fs::read_to_string(path).map_err(EnvrError::from)?;
    let mut doc: toml::Value = match toml::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return Ok(false),
    };
    let changed = migrate_legacy_settings_doc(&mut doc);
    if !changed {
        return Ok(false);
    }
    let backup = backup_path(path);
    fs::copy(path, &backup).map_err(EnvrError::from)?;
    let text =
        toml::to_string_pretty(&doc).map_err(|e| EnvrError::Runtime(format!("toml encode: {e}")))?;
    fs::write(path, text).map_err(EnvrError::from)?;
    Ok(true)
}

fn migrate_legacy_settings_doc(doc: &mut toml::Value) -> bool {
    let Some(root) = doc.as_table_mut() else {
        return false;
    };
    let mut changed = false;

    if let Some(v) = root.remove("runtime_root") {
        let paths = root
            .entry("paths")
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
        if let Some(p) = paths.as_table_mut()
            && !p.contains_key("runtime_root")
        {
            p.insert("runtime_root".to_string(), v);
            changed = true;
        }
    }

    if let Some(m) = root.get_mut("mirror").and_then(|v| v.as_table_mut()) {
        if let Some(v) = m.remove("strategy")
            && !m.contains_key("mode")
        {
            m.insert("mode".to_string(), v);
            changed = true;
        }
        if let Some(v) = m.remove("manual")
            && !m.contains_key("manual_id")
        {
            m.insert("manual_id".to_string(), v);
            changed = true;
        }
    }

    if let Some(d) = root.get_mut("download").and_then(|v| v.as_table_mut())
        && let Some(v) = d.remove("max_concurrent")
        && !d.contains_key("max_concurrent_downloads")
    {
        d.insert("max_concurrent_downloads".to_string(), v);
        changed = true;
    }

    let mut moved: Vec<(&str, toml::Value)> = Vec::new();
    for legacy in ["node", "python", "java", "go", "rust", "php", "deno", "bun"] {
        if let Some(v) = root.remove(legacy) {
            moved.push((legacy, v));
        }
    }
    let runtime = root
        .entry("runtime")
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    if let Some(rt) = runtime.as_table_mut() {
        for (legacy, v) in moved {
            if !rt.contains_key(legacy) {
                rt.insert(legacy.to_string(), v);
                changed = true;
            }
        }
    }

    changed
}

fn suggest_key_hint(key: &str) -> Option<&'static str> {
    match key {
        "runtime_root" => Some("paths.runtime_root"),
        "mirror.strategy" => Some("mirror.mode"),
        "mirror.manual" => Some("mirror.manual_id"),
        "download.max_concurrent" => Some("download.max_concurrent_downloads"),
        "node.mirror.url" => Some("runtime.node.download_source"),
        _ => None,
    }
}

fn config_writable_keys() -> &'static [&'static str] {
    &[
        "paths.runtime_root",
        "behavior.cleanup_downloads_after_install",
        "appearance.font.mode",
        "appearance.font.family",
        "appearance.theme_mode",
        "appearance.accent_color",
        "gui.downloads_panel.visible",
        "gui.downloads_panel.expanded",
        "gui.downloads_panel.x",
        "gui.downloads_panel.y",
        "gui.downloads_panel.x_frac",
        "gui.downloads_panel.y_frac",
        "download.max_concurrent_downloads",
        "download.retry_max",
        "mirror.mode",
        "mirror.manual_id",
        "i18n.locale",
        "runtime.node.download_source",
        "runtime.node.npm_registry_mode",
        "runtime.node.path_proxy_enabled",
        "runtime.python.download_source",
        "runtime.python.pip_registry_mode",
        "runtime.python.path_proxy_enabled",
        "runtime.java.current_distro",
        "runtime.java.download_source",
        "runtime.java.path_proxy_enabled",
        "runtime.go.download_source",
        "runtime.go.proxy_mode",
        "runtime.go.proxy_custom",
        "runtime.go.private_patterns",
        "runtime.go.path_proxy_enabled",
        "runtime.go.goproxy",
        "runtime.rust.download_source",
        "runtime.php.download_source",
        "runtime.php.windows_build",
        "runtime.php.path_proxy_enabled",
        "runtime.deno.download_source",
        "runtime.deno.package_source",
        "runtime.deno.path_proxy_enabled",
        "runtime.bun.package_source",
        "runtime.bun.path_proxy_enabled",
        "runtime.bun.global_bin_dir",
    ]
}
