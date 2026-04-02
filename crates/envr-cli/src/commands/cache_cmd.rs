use crate::cli::GlobalArgs;
use crate::commands::common;
use crate::output;

use envr_error::{EnvrError, EnvrResult};
use std::fs;
use std::path::PathBuf;

pub fn clean(g: &GlobalArgs, kind: Option<String>, all: bool) -> i32 {
    let root = match common::effective_runtime_root() {
        Ok(r) => r,
        Err(e) => return common::print_envr_error(g, e),
    };

    let target = match (
        all,
        kind.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty()),
    ) {
        (true, _) => root.join("cache"),
        (false, None) => root.join("cache"),
        (false, Some(k)) => root.join("cache").join(k.to_ascii_lowercase()),
    };

    match remove_dir_if_exists(&target) {
        Ok(()) => {
            let data = serde_json::json!({ "removed": target.to_string_lossy() });
            output::emit_ok(g, "cache_cleaned", data, || {
                if !g.quiet {
                    println!(
                        "{}",
                        crate::output::fmt_template(
                            &envr_core::i18n::tr_key(
                                "cli.cache.removed",
                                "已移除缓存：{path}",
                                "cache removed: {path}",
                            ),
                            &[("path", &target.display().to_string())],
                        )
                    );
                }
            })
        }
        Err(e) => common::print_envr_error(g, e),
    }
}

fn remove_dir_if_exists(path: &PathBuf) -> EnvrResult<()> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_file() {
        return Err(EnvrError::Validation(crate::output::fmt_template(
            &envr_core::i18n::tr_key(
                "cli.err.cache_path_is_file",
                "缓存路径是文件，应为目录：{path}",
                "cache path is a file, expected directory: {path}",
            ),
            &[("path", &path.display().to_string())],
        )));
    }
    fs::remove_dir_all(path).map_err(EnvrError::from)?;
    Ok(())
}
