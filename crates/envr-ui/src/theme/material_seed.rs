//! Linux: approximate Material 3 dynamic primary from GNOME accent (`accent-color`), when available.
//! Falls back via callers to [`crate::theme::tokens::base::BRAND_PRIMARY_MATERIAL_FALLBACK`].

use super::color::Srgb;

/// Reads `org.gnome.desktop.interface accent-color` and maps to sRGB. Returns `None` if unavailable.
pub fn linux_material_primary_seed() -> Option<Srgb> {
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
    #[cfg(target_os = "linux")]
    {
        gnome_accent_from_gsettings()
    }
}

#[cfg(target_os = "linux")]
fn gnome_accent_from_gsettings() -> Option<Srgb> {
    use std::process::Command;

    let out = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "accent-color"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    let name = s.trim().trim_matches(|c| c == '\'' || c == '"');
    gnome_accent_name_to_srgb(name)
}

/// GNOME 46+ preset names → representative sRGB (Material-like chroma).
#[cfg(target_os = "linux")]
fn gnome_accent_name_to_srgb(name: &str) -> Option<Srgb> {
    match name {
        "blue" => Srgb::from_hex("#0078D4").ok(),
        "teal" => Srgb::from_hex("#00897B").ok(),
        "green" => Srgb::from_hex("#2E7D32").ok(),
        "yellow" => Srgb::from_hex("#F9A825").ok(),
        "orange" => Srgb::from_hex("#EF6C00").ok(),
        "red" => Srgb::from_hex("#C62828").ok(),
        "pink" => Srgb::from_hex("#AD1457").ok(),
        "purple" => Srgb::from_hex("#6A1B9A").ok(),
        "slate" => Srgb::from_hex("#546E7A").ok(),
        _ => None,
    }
}
