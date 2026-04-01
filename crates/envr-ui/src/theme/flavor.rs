/// High-level visual family aligned with `refactor docs/03-gui-设计.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiFlavor {
    /// Windows — Fluent-inspired tokens (crisp, light elevation).
    #[default]
    Fluent,
    /// macOS — Liquid Glass–inspired tokens (softer, larger radius, cooler neutrals).
    LiquidGlass,
    /// Linux / cross-desktop — Material 3–inspired tokens (expressive primary, large corners).
    Material3,
}

impl UiFlavor {
    /// All variants for pickers and tests.
    pub const ALL: [Self; 3] = [Self::Fluent, Self::LiquidGlass, Self::Material3];

    /// Short stable key (logs, persistence).
    pub fn as_str(self) -> &'static str {
        match self {
            UiFlavor::Fluent => "fluent",
            UiFlavor::LiquidGlass => "liquid_glass",
            UiFlavor::Material3 => "material3",
        }
    }

    /// User-facing label (zh).
    pub fn label_zh(self) -> &'static str {
        match self {
            UiFlavor::Fluent => "Fluent（Windows）",
            UiFlavor::LiquidGlass => "Liquid Glass（macOS）",
            UiFlavor::Material3 => "Material 3（Linux）",
        }
    }

    /// User-facing label (en).
    pub fn label_en(self) -> &'static str {
        match self {
            UiFlavor::Fluent => "Fluent (Windows)",
            UiFlavor::LiquidGlass => "Liquid Glass (macOS)",
            UiFlavor::Material3 => "Material 3 (Linux)",
        }
    }
}

impl std::fmt::Display for UiFlavor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
