//! Heuristic **Groovy ↔ JDK** compatibility checks.
//!
//! Policy (current):
//! - Groovy 4.x+ requires Java 11+.
//! - Older lines default to Java 8+.

use crate::{kotlin_java::jdk_dir_label_effective_major, runtime::numeric_version_segments};

fn groovy_min_java_major(groovy_version: &str) -> u32 {
    let Some(parts) = numeric_version_segments(groovy_version) else {
        return 8;
    };
    let Some(&major) = parts.first() else {
        return 8;
    };
    if major >= 4 { 11 } else { 8 }
}

pub fn groovy_jdk_mismatch_message(groovy_version: &str, java_dir_label: &str) -> Option<String> {
    let min_java = groovy_min_java_major(groovy_version);
    let java_m = jdk_dir_label_effective_major(java_dir_label)?;
    if java_m < min_java {
        Some(format!(
            "Groovy {groovy_version} 与当前 JDK 不兼容：检测到的 JDK 主版本为 {java_m}，该 Groovy 发行在当前策略下需要 Java {min_java}+。请执行 `envr use java 21` 等更高主版本，或切换到更旧的 Groovy 主线；详见 docs/runtime/groovy.md。"
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groovy4_blocks_java8() {
        let msg = groovy_jdk_mismatch_message("4.0.31", "1.8.0_382").expect("msg");
        assert!(msg.contains("Java 11+"));
    }

    #[test]
    fn groovy4_allows_java11_and_21() {
        assert!(groovy_jdk_mismatch_message("4.0.31", "11.0.24+8").is_none());
        assert!(groovy_jdk_mismatch_message("4.0.31", "21.0.6+9").is_none());
    }

    #[test]
    fn groovy3_allows_java8() {
        assert!(groovy_jdk_mismatch_message("3.0.25", "1.8.0_382").is_none());
    }
}
