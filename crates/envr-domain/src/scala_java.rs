//! Heuristic **Scala 3 compiler ↔ JDK** compatibility using **version directory labels**
//! (e.g. `3.8.3`, `1.8.0_302`) — same signals as install paths and shims, no subprocess.
//!
//! Official Scala **3.3+** distributions ship a compiler built for **Java 17** (class file 61+);
//! running with Java 8 surfaces raw `UnsupportedClassVersionError`. Scala **3.0.x–3.2.x** releases
//! are treated as **Java 8+** for the directory-label heuristic (see `docs/runtime/scala.md`).

use super::kotlin_java::jdk_dir_label_effective_major;

/// Minimum Java **major** required to **run** the Scala 3 compiler for this install folder label.
///
/// Returns `None` for non–Scala-3 style labels (e.g. unexpected major).
pub fn scala_minimum_java_major_for_compiler(scala_version_label: &str) -> Option<u32> {
    let t = scala_version_label
        .trim()
        .trim_start_matches('v')
        .trim_start_matches('V');
    let mut parts = t.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    if major != 3 {
        return None;
    }
    let minor = parts.next()?.parse::<u64>().ok()?;
    if minor >= 3 { Some(17) } else { Some(8) }
}

/// `None` = OK; `Some` = user-facing message (CLI / shim / GUI) when JDK is **too old** for this Scala 3 build.
pub fn scala_jdk_mismatch_message(scala_version: &str, java_dir_label: &str) -> Option<String> {
    let java_m = jdk_dir_label_effective_major(java_dir_label)?;
    let need = scala_minimum_java_major_for_compiler(scala_version)?;
    if java_m < need {
        Some(format!(
            "Scala {scala_version} 的编译器需要 Java {need}+ 才能运行；当前 JDK 目录名推断主版本为 {java_m}。请执行 `envr use java 21` 等更高版本；详见 docs/runtime/scala.md。"
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scala_3_8_needs_java_17() {
        let msg = scala_jdk_mismatch_message("3.8.3", "1.8.0_302").expect("msg");
        assert!(msg.contains("3.8.3"));
        assert!(msg.contains("17"));
        assert!(msg.contains("8"));
    }

    #[test]
    fn scala_3_8_allows_java_21() {
        assert!(scala_jdk_mismatch_message("3.8.3", "21.0.6+9-LTS").is_none());
    }

    #[test]
    fn scala_3_2_allows_java_8() {
        assert!(scala_jdk_mismatch_message("3.2.2", "1.8.0_302").is_none());
    }

    #[test]
    fn scala_3_3_requires_17_not_11() {
        assert!(scala_jdk_mismatch_message("3.3.0", "11.0.20").is_some());
        assert!(scala_jdk_mismatch_message("3.3.0", "17.0.9").is_none());
    }
}
