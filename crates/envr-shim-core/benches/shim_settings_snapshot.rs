//! Quantify shim settings hot-path overhead:
//! legacy multi-load pattern vs single-snapshot pattern.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use envr_config::settings::{
    PhpWindowsBuildFlavor, Settings, bun_package_registry_env, deno_package_registry_env,
    reset_settings_load_caches,
};
use std::fs;
use tempfile::tempdir;

fn load_settings(path: &std::path::Path) -> Settings {
    Settings::load_or_default_from(path).expect("load settings")
}

fn legacy_multi_load(path: &std::path::Path) -> usize {
    // Mirrors the pre-optimization shim pattern: repeated load calls for each branch/env.
    let mut acc = 0usize;
    acc += usize::from(load_settings(path).runtime.node.path_proxy_enabled);
    acc += usize::from(load_settings(path).runtime.python.path_proxy_enabled);
    acc += usize::from(load_settings(path).runtime.java.path_proxy_enabled);
    acc += usize::from(load_settings(path).runtime.kotlin.path_proxy_enabled);
    acc += usize::from(load_settings(path).runtime.go.path_proxy_enabled);
    acc += usize::from(load_settings(path).runtime.php.path_proxy_enabled);
    acc += usize::from(load_settings(path).runtime.deno.path_proxy_enabled);
    acc += usize::from(load_settings(path).runtime.bun.path_proxy_enabled);
    acc += usize::from(matches!(
        load_settings(path).runtime.php.windows_build,
        PhpWindowsBuildFlavor::Ts
    ));
    acc += deno_package_registry_env(&load_settings(path)).len();
    acc += bun_package_registry_env(&load_settings(path)).len();
    acc
}

fn single_snapshot(path: &std::path::Path) -> usize {
    // New pattern: one settings snapshot for all decisions.
    let s = load_settings(path);
    let mut acc = 0usize;
    acc += usize::from(s.runtime.node.path_proxy_enabled);
    acc += usize::from(s.runtime.python.path_proxy_enabled);
    acc += usize::from(s.runtime.java.path_proxy_enabled);
    acc += usize::from(s.runtime.kotlin.path_proxy_enabled);
    acc += usize::from(s.runtime.go.path_proxy_enabled);
    acc += usize::from(s.runtime.php.path_proxy_enabled);
    acc += usize::from(s.runtime.deno.path_proxy_enabled);
    acc += usize::from(s.runtime.bun.path_proxy_enabled);
    acc += usize::from(matches!(
        s.runtime.php.windows_build,
        PhpWindowsBuildFlavor::Ts
    ));
    acc += deno_package_registry_env(&s).len();
    acc += bun_package_registry_env(&s).len();
    acc
}

fn bench_shim_settings_snapshot(c: &mut Criterion) {
    let tmp = tempdir().expect("tmp");
    let settings_path = tmp.path().join("settings.toml");
    fs::write(
        &settings_path,
        r#"
[runtime.node]
path_proxy_enabled = true
npm_registry_mode = "domestic"

[runtime.python]
path_proxy_enabled = true
pip_registry_mode = "official"

[runtime.java]
path_proxy_enabled = true

[runtime.kotlin]
path_proxy_enabled = true

[runtime.go]
path_proxy_enabled = true

[runtime.php]
path_proxy_enabled = true
windows_build = "ts"

[runtime.deno]
path_proxy_enabled = true
package_source = "domestic"

[runtime.bun]
path_proxy_enabled = true
package_source = "official"
"#,
    )
    .expect("write settings");

    let mut g = c.benchmark_group("shim_settings_hot_path");
    g.bench_function("legacy_multi_load_cold_process", |b| {
        b.iter(|| {
            // Simulate one shim process invocation.
            reset_settings_load_caches();
            black_box(legacy_multi_load(black_box(&settings_path)));
        });
    });
    g.bench_function("single_snapshot_cold_process", |b| {
        b.iter(|| {
            // Simulate one shim process invocation.
            reset_settings_load_caches();
            black_box(single_snapshot(black_box(&settings_path)));
        });
    });
    g.finish();
}

criterion_group!(benches, bench_shim_settings_snapshot);
criterion_main!(benches);
