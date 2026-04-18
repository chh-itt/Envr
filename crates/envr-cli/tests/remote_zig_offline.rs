//! Offline `envr remote zig` when `remote_latest_per_major_<plat>.json` is already seeded.
//!
//! Avoids network: `RuntimeService::try_load_remote_latest_per_major_from_disk` reads the cache
//! with no TTL gate for the snapshot path used by `remote`.

use assert_cmd::Command;
use serde_json::Value;
use std::fs;

const ENVELOPE_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/envelope.json"
));
const REMOTE_DATA_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/list_remote.json"
));

fn parse_json_line(stdout: &[u8]) -> Value {
    for line in stdout.split(|b| *b == b'\n') {
        if line.first() == Some(&b'{')
            && let Ok(v) = serde_json::from_slice::<Value>(line)
        {
            return v;
        }
    }
    panic!(
        "no json object in stdout: {}",
        String::from_utf8_lossy(stdout)
    );
}

fn assert_valid(schema_src: &str, instance: &Value) {
    let schema_src = schema_src.trim_start_matches('\u{feff}');
    let schema: Value = serde_json::from_str(schema_src).expect("schema JSON");
    if let Err(e) = jsonschema::validate(&schema, instance) {
        panic!("schema validation failed: {e}");
    }
}

#[test]
fn remote_zig_uses_prefetched_remote_latest_cache_without_network() {
    let plat = match envr_runtime_zig::zig_json_platform_key() {
        Ok(p) => p,
        Err(_) => {
            // Host OS/arch has no mapped official Zig triple in this build.
            return;
        }
    };

    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path();
    let cache_zig = runtime_root.join("cache").join("zig");
    fs::create_dir_all(&cache_zig).expect("cache dir");
    let cache_path = cache_zig.join(format!("remote_latest_per_major_{plat}.json"));
    fs::write(&cache_path, "[\"0.14.1\",\"0.14.0\"]\n").expect("write zig remote cache");

    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .args(["--format", "json", "remote", "zig"])
        .output()
        .expect("run envr");

    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_eq!(v.get("code"), Some(&serde_json::json!("list_remote")));
    let data = v.get("data").expect("data");
    assert_valid(REMOTE_DATA_SCHEMA, data);
    let runtimes = data
        .get("remote_runtimes")
        .and_then(|x| x.as_array())
        .expect("remote_runtimes");
    assert_eq!(runtimes.len(), 1);
    let versions = runtimes[0]
        .get("versions")
        .and_then(|x| x.as_array())
        .expect("versions");
    let got: Vec<&str> = versions
        .iter()
        .filter_map(|o| o.get("version").and_then(|x| x.as_str()))
        .collect();
    assert_eq!(got, vec!["0.14.1", "0.14.0"]);
    assert_eq!(
        data.get("cached_snapshot"),
        Some(&serde_json::json!(true)),
        "expected snapshot from disk, not live fetch"
    );
    assert_eq!(
        data.get("remote_refreshing"),
        Some(&serde_json::json!(true)),
        "prefetched cache currently triggers background refresh by design"
    );
    assert_eq!(
        data.get("prefix_fallback"),
        Some(&serde_json::json!(false)),
        "non-prefix query should not fallback when cache is present"
    );
}
