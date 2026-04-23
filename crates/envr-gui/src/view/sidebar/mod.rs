use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};

use envr_ui::theme::ThemeTokens;

use crate::app::{Message, Route};
use crate::icons::Lucide;
use crate::theme as gui_theme;
use crate::widget_styles::{ButtonVariant, button_style};

fn route_icon(route: Route) -> Lucide {
    match route {
        Route::Dashboard => Lucide::LayoutDashboard,
        Route::Runtime => Lucide::Package,
        Route::RuntimeConfig => Lucide::Settings,
        Route::Downloads => Lucide::Download,
        Route::Settings => Lucide::Settings,
        Route::About => Lucide::Info,
    }
}

pub fn sidebar(current: Route, tokens: ThemeTokens) -> Element<'static, Message> {
    let panel = gui_theme::panel_container_style(tokens);
    let sp = tokens.space();
    let txt = gui_theme::to_color(tokens.colors.text_muted);
    let mut col = column![].spacing(sp.sm as f32);
    for route in Route::ALL {
        let selected = route == current;
        let icon_c = if selected {
            gui_theme::contrast_on_primary(tokens)
        } else {
            txt
        };
        let icn_sz = if selected { 18.0 } else { 16.0 };
        let label = row![route_icon(route).view(icn_sz, icon_c), text(route.label()),]
            .spacing(sp.sm as f32)
            .align_y(Alignment::Center);
        let variant = if selected {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        };
        let h = if selected {
            tokens.control_height_primary
        } else {
            tokens.control_height_secondary
        }
        .max(tokens.min_click_target_px());
        let b = button(label)
            .on_press(Message::Navigate(route))
            .width(Length::Fill)
            .height(Length::Fixed(h))
            .padding([sp.sm as f32, (sp.sm + 2) as f32])
            .style(button_style(tokens, variant));
        col = col.push(b);
    }
    container(col.width(Length::Fixed(tokens.sidebar_width())))
        .padding(sp.sm + 2)
        .style(move |theme: &Theme| panel(theme))
        .into()
}
