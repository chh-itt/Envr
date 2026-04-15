//! Criterion benchmarks for upward `.envr.toml` discovery + parse (see `load_project_config`).

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use envr_config::project_config::{
    PROJECT_CONFIG_FILE, load_project_config_profile, reset_project_config_load_cache,
};
use std::fs;
use std::path::PathBuf;

fn deep_leaf_under(root: &std::path::Path, depth: usize) -> PathBuf {
    let mut p = root.to_path_buf();
    for i in 0..depth {
        p.push(format!("seg_{i}"));
    }
    fs::create_dir_all(&p).expect("mkdir");
    p
}

fn project_config_benchmarks(c: &mut Criterion) {
    let tmp = tempfile::tempdir().expect("tmp");
    let root = tmp.path();
    let leaf = deep_leaf_under(root, 24);
    fs::write(
        root.join(PROJECT_CONFIG_FILE),
        r#"
[env]
FOO = "root"

[runtimes.node]
version = "20"
"#,
    )
    .expect("write");

    let mut g = c.benchmark_group("project_config");
    g.bench_function("load_profile_cold", |b| {
        b.iter(|| {
            reset_project_config_load_cache();
            black_box(
                load_project_config_profile(black_box(&leaf), None)
                    .expect("load")
                    .expect("found"),
            );
        });
    });
    g.bench_function("load_profile_warm", |b| {
        reset_project_config_load_cache();
        let _ = load_project_config_profile(&leaf, None).expect("prime");
        b.iter(|| {
            black_box(
                load_project_config_profile(black_box(&leaf), None)
                    .expect("load")
                    .expect("found"),
            );
        });
    });
    g.finish();
}

criterion_group!(benches, project_config_benchmarks);
criterion_main!(benches);
