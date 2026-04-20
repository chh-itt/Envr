//! Shared compatibility helpers for JVM-hosted runtimes (Kotlin, Scala, ...).

use crate::{clojure_java, kotlin_java, scala_java};

/// User-facing mismatch message for hosted-runtime/JDK combination.
///
/// Returns `None` when the runtime is not in the JVM-hosted compatibility table or
/// when the combination is considered compatible.
pub fn hosted_runtime_jdk_mismatch_message(
    runtime_key: &str,
    runtime_version_label: &str,
    java_dir_label: &str,
) -> Option<String> {
    match runtime_key {
        "kotlin" => kotlin_java::kotlin_jdk_mismatch_message(runtime_version_label, java_dir_label),
        "scala" => scala_java::scala_jdk_mismatch_message(runtime_version_label, java_dir_label),
        "clojure" => clojure_java::clojure_jdk_mismatch_message(runtime_version_label, java_dir_label),
        _ => None,
    }
}

/// True when runtime participates in JVM-hosted Java-env merge/check flow.
pub fn is_jvm_hosted_runtime(runtime_key: &str) -> bool {
    matches!(runtime_key, "kotlin" | "scala" | "clojure")
}
