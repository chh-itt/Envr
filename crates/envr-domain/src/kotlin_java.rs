//! Heuristic **Kotlin compiler ↔ JDK** compatibility using **version directory labels**
//! (e.g. `21.0.6+9-LTS`, `2.0.21`) — same signals as install paths and shims, no subprocess.

use crate::runtime::numeric_version_segments;

/// Leading unsigned integer in a JDK directory segment (`6` from `6+9-LTS`, `0` from `0_302`).
fn jdk_segment_leading_u64(segment: &str) -> Option<u64> {
    let mut n = 0u64;
    let mut any = false;
    for c in segment.chars() {
        if !c.is_ascii_digit() {
            break;
        }
        any = true;
        n = n.saturating_mul(10).saturating_add((c as u8 - b'0') as u64);
    }
    any.then_some(n)
}

/// Numeric segments for **JDK install folder names** — tolerates `+` build metadata and `_` suffixes.
fn jdk_dir_label_numeric_segments(label: &str) -> Option<Vec<u64>> {
    let t = label.trim().trim_start_matches('v');
    if t.is_empty() {
        return None;
    }
    let before_metadata = t.split('+').next()?;
    let mut parts = Vec::new();
    for seg in before_metadata.split('.') {
        let n = jdk_segment_leading_u64(seg)?;
        parts.push(n);
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts)
}

/// Effective Java **major** from the JDK install directory name under `.../java/versions/.../<label>`.
pub fn jdk_dir_label_effective_major(label: &str) -> Option<u32> {
    let parts = jdk_dir_label_numeric_segments(label)?;
    if parts.first() == Some(&1) {
        return parts.get(1).copied().map(|m| m as u32);
    }
    parts.first().copied().map(|m| m as u32)
}

/// When `Some(n)`, JDK majors **strictly greater than `n`** are treated as incompatible with this
/// Kotlin **compiler** bundle (bundled IntelliJ `JavaVersion` may not parse the JDK yet).
///
/// Policy is intentionally conservative and version-table driven only where we have concrete
/// field reports (Kotlin `2.0.x` + JDK **25+** startup failures).
pub fn kotlin_compiler_bundle_max_supported_java_major(kotlin_version: &str) -> Option<u32> {
    let parts = numeric_version_segments(kotlin_version)?;
    let ma = *parts.first()?;
    let mi = parts.get(1).copied().unwrap_or(0);
    if ma == 2 && mi == 0 {
        // Kotlin 2.0.x distributions: JDK 25+ can crash inside bundled `JavaVersion.parse`.
        return Some(24);
    }
    None
}

/// `None` = OK; `Some` = user-facing message (CLI / shim / GUI).
pub fn kotlin_jdk_mismatch_message(kotlin_version: &str, java_dir_label: &str) -> Option<String> {
    let java_m = jdk_dir_label_effective_major(java_dir_label)?;
    let Some(cap) = kotlin_compiler_bundle_max_supported_java_major(kotlin_version) else {
        return None;
    };
    if java_m > cap {
        Some(format!(
            "Kotlin {kotlin_version} 与当前 JDK 不兼容：检测到的 JDK 主版本为 {java_m}，而该编译器发行在当前策略下仅支持 Java ≤{cap}（JDK {} 起可能无法启动 kotlinc）。请执行 `envr use java 21` 等较低主版本，或升级到更新的 Kotlin 主线；详见 docs/runtime/kotlin.md。",
            cap + 1
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jdk_label_parses_java8_style() {
        assert_eq!(jdk_dir_label_effective_major("1.8.0_302"), Some(8));
    }

    #[test]
    fn jdk_label_parses_modern() {
        assert_eq!(
            jdk_dir_label_effective_major("21.0.6+9-LTS"),
            Some(21)
        );
    }

    #[test]
    fn kotlin_2_0_blocks_java_25() {
        let msg = kotlin_jdk_mismatch_message("2.0.21", "25.0.1").expect("msg");
        assert!(msg.contains("2.0.21"));
        assert!(msg.contains("25"));
    }

    #[test]
    fn kotlin_2_0_allows_java_21() {
        assert!(kotlin_jdk_mismatch_message("2.0.21", "21.0.6+9-LTS").is_none());
    }
}
