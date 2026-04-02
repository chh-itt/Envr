//! Lucide icons bundled under `assets/icons/lucide/` (MIT).
//! Refresh from upstream: `scripts/fetch_lucide_icons.ps1`.

use iced::widget::svg::{self, Svg};
use iced::{Color, Element, Length};

macro_rules! lucide_bytes {
    ($file:literal) => {
        include_bytes!(concat!("../assets/icons/lucide/", $file))
    };
}

/// Named Lucide glyphs used in the shell.
#[derive(Debug, Clone, Copy)]
pub enum Lucide {
    LayoutDashboard,
    RefreshCw,
    Settings,
    Download,
    ChevronsUpDown,
    EyeOff,
    CircleAlert,
    Package,
    /// Reserved for sidebar / panel affordances (asset kept with Lucide bundle).
    #[allow(dead_code)]
    PanelLeftOpen,
    X,
    Menu,
    Info,
}

impl Lucide {
    pub fn svg_bytes(self) -> &'static [u8] {
        match self {
            Lucide::LayoutDashboard => lucide_bytes!("layout-dashboard.svg"),
            Lucide::RefreshCw => lucide_bytes!("refresh-cw.svg"),
            Lucide::Settings => lucide_bytes!("settings.svg"),
            Lucide::Download => lucide_bytes!("download.svg"),
            Lucide::ChevronsUpDown => lucide_bytes!("chevrons-up-down.svg"),
            Lucide::EyeOff => lucide_bytes!("eye-off.svg"),
            Lucide::CircleAlert => lucide_bytes!("circle-alert.svg"),
            Lucide::Package => lucide_bytes!("package.svg"),
            Lucide::PanelLeftOpen => lucide_bytes!("panel-left-open.svg"),
            Lucide::X => lucide_bytes!("x.svg"),
            Lucide::Menu => lucide_bytes!("menu.svg"),
            Lucide::Info => lucide_bytes!("info.svg"),
        }
    }

    fn handle(self) -> svg::Handle {
        svg::Handle::from_memory(self.svg_bytes())
    }

    /// Fixed-size symbolic icon (`currentColor` stroke is tinted via SVG style).
    pub fn view<Message: 'static>(self, px: f32, color: Color) -> Element<'static, Message> {
        Svg::new(self.handle())
            .width(Length::Fixed(px))
            .height(Length::Fixed(px))
            .style(move |_theme, _status| svg::Style { color: Some(color) })
            .into()
    }
}
