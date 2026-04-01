use super::flavor::UiFlavor;

/// Picks the design doc default for the **compile target** OS.
pub fn default_flavor_for_target() -> UiFlavor {
    if cfg!(target_os = "windows") {
        UiFlavor::Fluent
    } else if cfg!(target_os = "macos") {
        UiFlavor::LiquidGlass
    } else {
        UiFlavor::Material3
    }
}
