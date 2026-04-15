//! Bench resolve path with and without per-invocation settings reload.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use envr_shim_core::{
    CoreCommand, ShimContext, load_shim_settings_snapshot, resolve_core_shim_command_with_settings,
};
use std::fs;
use tempfile::tempdir;

fn setup_node_home(root: &std::path::Path) {
    let versions = root.join("runtimes/node/versions");
    let home = versions.join("20.0.0");
    #[cfg(windows)]
    let node_bin = home.join("node.exe");
    #[cfg(not(windows))]
    let node_bin = home.join("bin/node");

    if let Some(parent) = node_bin.parent() {
        fs::create_dir_all(parent).expect("create node parent");
    }
    fs::write(&node_bin, []).expect("write node executable");

    let current = root.join("runtimes/node/current");
    if let Some(parent) = current.parent() {
        fs::create_dir_all(parent).expect("create node root");
    }
    #[cfg(windows)]
    fs::write(&current, home.display().to_string()).expect("write current pointer");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&home, &current).expect("create current symlink");
}

fn bench_resolve_core_shim_command(c: &mut Criterion) {
    let tmp = tempdir().expect("tmp");
    let root = tmp.path();
    let working_dir = root.join("prj");
    fs::create_dir_all(&working_dir).expect("create working dir");
    setup_node_home(root);

    let ctx = ShimContext::with_runtime_root(root.to_path_buf(), working_dir, None);
    let preloaded = load_shim_settings_snapshot();

    let mut g = c.benchmark_group("resolve_core_shim_command");
    g.bench_function("legacy_load_settings_each_invocation", |b| {
        b.iter(|| {
            let settings = load_shim_settings_snapshot();
            black_box(
                resolve_core_shim_command_with_settings(
                    CoreCommand::Node,
                    black_box(&ctx),
                    black_box(&settings),
                )
                .expect("resolve"),
            );
        });
    });
    g.bench_function("preloaded_settings_snapshot", |b| {
        b.iter(|| {
            black_box(
                resolve_core_shim_command_with_settings(
                    CoreCommand::Node,
                    black_box(&ctx),
                    black_box(&preloaded),
                )
                .expect("resolve"),
            );
        });
    });
    g.finish();
}

criterion_group!(benches, bench_resolve_core_shim_command);
criterion_main!(benches);
