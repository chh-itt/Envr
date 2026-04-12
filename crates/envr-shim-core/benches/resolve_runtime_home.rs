//! Criterion benchmarks for [`envr_shim_core::resolve_runtime_home_for_lang`] (pin resolution + project config).

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use envr_config::project_config::reset_project_config_load_cache;
use envr_shim_core::{ShimContext, resolve_runtime_home_for_lang};
use std::fs;
use tempfile::tempdir;

fn setup_node_version(root: &std::path::Path, ver: &str) {
    let home = root
        .join("runtimes")
        .join("node")
        .join("versions")
        .join(ver);
    fs::create_dir_all(home.join("bin")).expect("mkdir");
    #[cfg(windows)]
    fs::write(home.join("bin").join("node.exe"), b"").expect("node.exe");
    #[cfg(not(windows))]
    fs::write(home.join("bin").join("node"), b"").expect("node");
}

fn bench_resolve_node_home(c: &mut Criterion) {
    let tmp = tempdir().expect("tmp");
    let root = tmp.path();
    setup_node_version(root, "20.0.0");
    let ctx = ShimContext::with_runtime_root(root.to_path_buf(), root.to_path_buf(), None);

    let mut g = c.benchmark_group("resolve_runtime_home_for_lang");
    g.bench_function("node_pinned_cold_project_cache", |b| {
        b.iter(|| {
            reset_project_config_load_cache();
            black_box(
                resolve_runtime_home_for_lang(black_box(&ctx), "node", Some("20.0.0"))
                    .expect("resolve"),
            );
        });
    });
    g.bench_function("node_pinned_warm", |b| {
        reset_project_config_load_cache();
        let _ = resolve_runtime_home_for_lang(&ctx, "node", Some("20.0.0")).expect("prime");
        b.iter(|| {
            black_box(
                resolve_runtime_home_for_lang(black_box(&ctx), "node", Some("20.0.0"))
                    .expect("resolve"),
            );
        });
    });
    g.finish();
}

criterion_group!(benches, bench_resolve_node_home);
criterion_main!(benches);
