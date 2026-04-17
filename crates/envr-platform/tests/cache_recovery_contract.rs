use std::fs;

#[test]
fn cache_recovery_removes_corrupted_json() {
    let tmp = tempfile::TempDir::new().expect("tmp");
    let p = tmp.path().join("cache.json");
    fs::write(&p, b"{ nope").expect("write");

    let got = envr_platform::cache_recovery::read_json_string_list(&p, None, |_| true);
    assert!(got.is_none());
    assert!(!p.exists(), "corrupted cache must be removed");
}

#[test]
fn cache_recovery_removes_low_quality_cache() {
    let tmp = tempfile::TempDir::new().expect("tmp");
    let p = tmp.path().join("cache.json");
    fs::write(&p, br#"["1","2","3"]"#).expect("write");

    // Simulate Python's legacy/low-quality heuristic (e.g. major-only grouping).
    let got = envr_platform::cache_recovery::read_json_string_list(&p, None, |xs| xs.len() >= 6);
    assert!(got.is_none());
    assert!(!p.exists(), "invalid cache must be removed");
}
