//! Bench `package.json` engines.node extraction paths.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use serde_json::Value;
use std::fs;
use std::io::Read;
use tempfile::tempdir;

fn read_engines_node_legacy(pkg: &std::path::Path) -> Option<String> {
    let mut s = String::new();
    fs::File::open(pkg).ok()?.read_to_string(&mut s).ok()?;
    let v: Value = serde_json::from_str(&s).ok()?;
    let engines = v.get("engines")?;
    let node = engines.get("node")?;
    let Value::String(spec) = node else {
        return None;
    };
    let spec = spec.trim();
    if spec.is_empty() {
        None
    } else {
        Some(spec.to_string())
    }
}

fn read_engines_node_prefilter(pkg: &std::path::Path) -> Option<String> {
    let mut s = String::new();
    fs::File::open(pkg).ok()?.read_to_string(&mut s).ok()?;
    if !(s.contains("\"engines\"") && s.contains("\"node\"")) {
        return None;
    }
    let v: Value = serde_json::from_str(&s).ok()?;
    let engines = v.get("engines")?;
    let node = engines.get("node")?;
    let Value::String(spec) = node else {
        return None;
    };
    let spec = spec.trim();
    if spec.is_empty() {
        None
    } else {
        Some(spec.to_string())
    }
}

fn bench_node_engines_hint(c: &mut Criterion) {
    let tmp = tempdir().expect("tmp");
    let pkg_without_engines = tmp.path().join("package-no-engines.json");
    let pkg_with_engines = tmp.path().join("package-with-engines.json");

    let mut large_deps = String::new();
    for i in 0..300 {
        if i > 0 {
            large_deps.push(',');
        }
        large_deps.push_str(&format!("\"dep-{i}\":\"^1.0.0\""));
    }

    fs::write(
        &pkg_without_engines,
        format!(
            "{{\"name\":\"bench\",\"version\":\"1.0.0\",\"dependencies\":{{{large_deps}}}}}"
        ),
    )
    .expect("write package without engines");
    fs::write(
        &pkg_with_engines,
        format!(
            "{{\"name\":\"bench\",\"version\":\"1.0.0\",\"engines\":{{\"node\":\"^20\"}},\"dependencies\":{{{large_deps}}}}}"
        ),
    )
    .expect("write package with engines");

    let mut g = c.benchmark_group("node_engines_hint_read");
    g.bench_function("legacy_without_engines", |b| {
        b.iter(|| {
            black_box(read_engines_node_legacy(black_box(&pkg_without_engines)));
        });
    });
    g.bench_function("prefilter_without_engines", |b| {
        b.iter(|| {
            black_box(read_engines_node_prefilter(black_box(&pkg_without_engines)));
        });
    });
    g.bench_function("legacy_with_engines", |b| {
        b.iter(|| {
            black_box(read_engines_node_legacy(black_box(&pkg_with_engines)));
        });
    });
    g.bench_function("prefilter_with_engines", |b| {
        b.iter(|| {
            black_box(read_engines_node_prefilter(black_box(&pkg_with_engines)));
        });
    });
    g.finish();
}

criterion_group!(benches, bench_node_engines_hint);
criterion_main!(benches);
