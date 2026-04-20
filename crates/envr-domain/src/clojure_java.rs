//! Heuristic **Clojure CLI tools ↔ JDK** compatibility checks.
//!
//! Current policy is intentionally minimal: Clojure CLI requires Java 8+.

use crate::kotlin_java::jdk_dir_label_effective_major;

pub fn clojure_jdk_mismatch_message(clojure_version: &str, java_dir_label: &str) -> Option<String> {
    let java_m = jdk_dir_label_effective_major(java_dir_label)?;
    if java_m < 8 {
        Some(format!(
            "Clojure {clojure_version} 需要 Java 8+。检测到的 JDK 主版本为 {java_m}。请执行 `envr use java 21` 等更高版本；详见 docs/runtime/clojure.md。"
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clojure_blocks_java7() {
        let msg = clojure_jdk_mismatch_message("1.12.4.1629", "1.7.0_80").expect("msg");
        assert!(msg.contains("8+"));
    }

    #[test]
    fn clojure_allows_java8_and_21() {
        assert!(clojure_jdk_mismatch_message("1.12.4.1629", "1.8.0_302").is_none());
        assert!(clojure_jdk_mismatch_message("1.12.4.1629", "21.0.6+9-LTS").is_none());
    }
}
