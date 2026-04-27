use serde::{Deserialize, Serialize};

use super::defaults;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontMode {
    Auto,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    FollowSystem,
    Light,
    Dark,
}

impl Default for ThemeMode {
    fn default() -> Self {
        defaults::theme_mode()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocaleMode {
    FollowSystem,
    ZhCn,
    EnUs,
}

impl Default for LocaleMode {
    fn default() -> Self {
        defaults::locale_mode()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontSettings {
    #[serde(default = "defaults::font_mode")]
    pub mode: FontMode,

    /// Used only when `mode = "custom"`.
    #[serde(default)]
    pub family: Option<String>,
}

impl Default for FontSettings {
    fn default() -> Self {
        Self {
            mode: defaults::font_mode(),
            family: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AppearanceSettings {
    #[serde(default)]
    pub font: FontSettings,

    #[serde(default = "defaults::theme_mode")]
    pub theme_mode: ThemeMode,

    /// Optional brand accent `#RGB` / `#RRGGBB`; merged into theme primary when valid (GUI-003).
    #[serde(default)]
    pub accent_color: Option<String>,
}

/// Order and visibility for runtime hub + dashboard overview (string keys = `RuntimeDescriptor::key`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RuntimeLayoutSettings {
    /// Permutation of runtime keys; empty means built-in default order at resolve time.
    #[serde(default)]
    pub order: Vec<String>,
    /// Keys hidden from the runtime hub and shown only in the dashboard “hidden” region.
    #[serde(default)]
    pub hidden: Vec<String>,
}

/// GUI-only state persisted in `settings.toml` so window layout/UX preferences survive restarts.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct GuiSettings {
    #[serde(default)]
    pub downloads_panel: DownloadsPanelSettings,

    #[serde(default)]
    pub runtime_layout: RuntimeLayoutSettings,

    /// If enabled, GUI startup performs a best-effort unified remote cache warm-up when stale.
    #[serde(default = "defaults::gui_runtime_cache_auto_update_on_launch")]
    pub runtime_cache_auto_update_on_launch: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DownloadsPanelSettings {
    /// Whether the floating downloads panel is visible.
    #[serde(default = "defaults::downloads_panel_visible")]
    pub visible: bool,
    /// Whether the panel is expanded (shows job list).
    #[serde(default = "defaults::downloads_panel_expanded")]
    pub expanded: bool,
    /// Left offset in pixels from the window's left edge.
    #[serde(default = "defaults::downloads_panel_x")]
    pub x: i32,
    /// Bottom offset in pixels from the window's bottom edge.
    #[serde(default = "defaults::downloads_panel_y")]
    pub y: i32,
    /// Normalized horizontal inset: `x ≈ x_frac * (client_w - 2*pad - panel_w)` (`tasks_gui.md` GUI-061).
    #[serde(default)]
    pub x_frac: Option<f32>,
    /// Normalized bottom inset: `y ≈ y_frac * (client_h - 2*pad)` (`tasks_gui.md` GUI-061).
    #[serde(default)]
    pub y_frac: Option<f32>,
}

impl DownloadsPanelSettings {
    /// Pixel insets for the panel, using fractional coords when present (DPI / resize stable).
    pub fn pixel_insets(
        &self,
        client_w: f32,
        client_h: f32,
        content_pad: f32,
        panel_w: f32,
    ) -> (i32, i32) {
        let inner_w = (client_w - 2.0 * content_pad).max(1.0);
        let inner_h = (client_h - 2.0 * content_pad).max(1.0);
        let avail_x = (inner_w - panel_w).max(1.0);
        if let (Some(xf), Some(yf)) = (self.x_frac, self.y_frac) {
            let x = (xf.clamp(0.0, 1.0) * avail_x).round() as i32;
            let y = (yf.clamp(0.0, 1.0) * inner_h).round() as i32;
            (x.max(0), y.max(0))
        } else {
            (self.x.max(0), self.y.max(0))
        }
    }

    /// Writes [`Self::x_frac`] / [`Self::y_frac`] from current pixel offsets (for persistence).
    pub fn sync_frac_from_pixels(
        &mut self,
        x: i32,
        y: i32,
        client_w: f32,
        client_h: f32,
        content_pad: f32,
        panel_w: f32,
    ) {
        let inner_w = (client_w - 2.0 * content_pad).max(1.0);
        let inner_h = (client_h - 2.0 * content_pad).max(1.0);
        let avail_x = (inner_w - panel_w).max(1.0);
        self.x = x.max(0);
        self.y = y.max(0);
        self.x_frac = Some((self.x as f32 / avail_x).clamp(0.0, 1.0));
        self.y_frac = Some((self.y as f32 / inner_h).clamp(0.0, 1.0));
    }
}

impl Default for DownloadsPanelSettings {
    fn default() -> Self {
        Self {
            visible: defaults::downloads_panel_visible(),
            expanded: defaults::downloads_panel_expanded(),
            x: defaults::downloads_panel_x(),
            y: defaults::downloads_panel_y(),
            x_frac: None,
            y_frac: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct I18nSettings {
    #[serde(default = "defaults::locale_mode")]
    pub locale: LocaleMode,
}
