//! OS / environment accessibility hints (`tasks_gui.md` GUI-052).

/// `ENVR_REDUCE_MOTION=1` or `ACCESSIBILITY_REDUCED_MOTION=1` forces reduced motion (tests & CI).
fn reduced_motion_from_env() -> bool {
    matches!(std::env::var("ENVR_REDUCE_MOTION").as_deref(), Ok("1"))
        || matches!(
            std::env::var("ACCESSIBILITY_REDUCED_MOTION").as_deref(),
            Ok("1")
        )
}

#[cfg(windows)]
fn windows_prefers_reduced_ui_effects() -> bool {
    use std::ffi::c_void;
    /// `SPI_GETUIEFFECTS` — `FALSE` means UI effects (including many animations) are off.
    const SPI_GETUIEFFECTS: u32 = 0x2018;
    #[link(name = "user32")]
    unsafe extern "system" {
        fn SystemParametersInfoW(
            ui_action: u32,
            ui_param: u32,
            pv_param: *mut c_void,
            f_win_ini: u32,
        ) -> i32;
    }
    let mut enabled: i32 = 1;
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETUIEFFECTS,
            0,
            &mut enabled as *mut i32 as *mut c_void,
            0,
        )
    };
    if ok == 0 {
        return false;
    }
    enabled == 0
}

/// Best-effort: system or env prefers shorter / no UI transitions (GUI-052).
///
/// - **Windows**: `SPI_GETUIEFFECTS` off ⇒ treat as reduced motion.
/// - **Other OS**: env vars only (future: macOS `NSWorkspace`, XDG / GTK as needed).
pub fn prefers_reduced_motion() -> bool {
    if reduced_motion_from_env() {
        return true;
    }
    #[cfg(windows)]
    {
        windows_prefers_reduced_ui_effects()
    }
    #[cfg(not(windows))]
    {
        false
    }
}
