#[cfg(target_os = "windows")]
use std::path::{Path, PathBuf};

pub fn preferred_system_sans_family() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        // Prefer YaHei UI for CJK coverage, fallback to Segoe UI.
        if windows_font_exists("msyh.ttc") {
            return "Microsoft YaHei UI";
        }
        "Segoe UI"
    }

    #[cfg(target_os = "macos")]
    {
        // Modern macOS CJK system font.
        return "PingFang SC";
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // Prefer common CJK-capable sans fonts.
        return "Noto Sans CJK SC";
    }
}

pub fn font_candidates() -> &'static [&'static str] {
    #[cfg(target_os = "windows")]
    {
        &[
            "Microsoft YaHei UI",
            "Microsoft YaHei",
            "Segoe UI",
            "SimSun",
            "SimHei",
            "KaiTi",
            "Noto Sans CJK SC",
        ]
    }

    #[cfg(target_os = "macos")]
    {
        &[
            "PingFang SC",
            "PingFang TC",
            "Hiragino Sans GB",
            "Heiti SC",
            "Helvetica Neue",
            "Noto Sans CJK SC",
        ]
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        &[
            "Noto Sans CJK SC",
            "Noto Sans CJK",
            "WenQuanYi Micro Hei",
            "Source Han Sans SC",
            "DejaVu Sans",
            "Liberation Sans",
        ]
    }
}

#[cfg(target_os = "windows")]
fn windows_font_exists(file_name: &str) -> bool {
    let windir = std::env::var("WINDIR")
        .or_else(|_| std::env::var("SystemRoot"))
        .unwrap_or_else(|_| "C:\\Windows".to_string());

    let p = PathBuf::from(windir).join("Fonts").join(file_name);
    Path::new(&p).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferred_sans_is_non_empty() {
        assert!(!preferred_system_sans_family().is_empty());
    }

    #[test]
    fn font_candidates_lists_platform_stack() {
        let c = font_candidates();
        assert!(!c.is_empty());
        assert!(c.iter().all(|s| !s.is_empty()));
    }
}
