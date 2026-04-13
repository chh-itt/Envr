use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use envr_config::settings::ThemeMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiScheme {
    Light,
    Dark,
}

pub fn scheme_for_mode(mode: ThemeMode) -> UiScheme {
    match mode {
        ThemeMode::Light => UiScheme::Light,
        ThemeMode::Dark => UiScheme::Dark,
        ThemeMode::FollowSystem => {
            if system_prefers_dark_cached() {
                UiScheme::Dark
            } else {
                UiScheme::Light
            }
        }
    }
}

struct Cache {
    /// `None` until the first successful probe — avoids `Instant - Duration` underflow
    /// when the process has been running for less than the “fake age” we’d subtract.
    last_check: Option<Instant>,
    prefers_dark: bool,
}

pub fn system_prefers_dark_cached() -> bool {
    static C: OnceLock<Mutex<Cache>> = OnceLock::new();
    let now = Instant::now();

    let mut g = C
        .get_or_init(|| {
            Mutex::new(Cache {
                last_check: None,
                prefers_dark: false,
            })
        })
        .lock()
        .expect("theme scheme cache lock");

    if let Some(prev) = g.last_check
        && now.duration_since(prev) < Duration::from_millis(900)
    {
        return g.prefers_dark;
    }

    let v = detect_system_prefers_dark();
    g.last_check = Some(now);
    g.prefers_dark = v;
    v
}

fn detect_system_prefers_dark() -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;

        // Read `AppsUseLightTheme` from registry. 0 = dark, 1 = light.
        let out = Command::new("reg")
            .args([
                "query",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
                "/v",
                "AppsUseLightTheme",
            ])
            .output();

        let Ok(out) = out else { return false };
        if !out.status.success() {
            return false;
        }
        let s = String::from_utf8_lossy(&out.stdout);
        // Example: "AppsUseLightTheme    REG_DWORD    0x0"
        if let Some(idx) = s.rfind("0x") {
            let hex = s[idx..].trim();
            return hex.starts_with("0x0");
        }
        false
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        // `defaults read -g AppleInterfaceStyle` prints "Dark" when dark mode is enabled.
        let out = Command::new("defaults")
            .args(["read", "-g", "AppleInterfaceStyle"])
            .output();
        let Ok(out) = out else { return false };
        if !out.status.success() {
            return false;
        }
        let s = String::from_utf8_lossy(&out.stdout);
        s.to_ascii_lowercase().contains("dark")
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use std::process::Command;
        // Try GNOME `gsettings` first.
        let out = Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "color-scheme"])
            .output();
        if let Ok(out) = out
            && out.status.success()
        {
            let s = String::from_utf8_lossy(&out.stdout);
            // 'prefer-dark'
            return s.to_ascii_lowercase().contains("dark");
        }
        false
    }
}
