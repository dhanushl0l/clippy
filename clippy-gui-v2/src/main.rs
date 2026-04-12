use clippy::APP_ID;
use clippy_gui_v2::state::ClipboardState;
use clippy_gui_v2::update::update;
use clippy_gui_v2::view::view;

use iced::theme::Palette;
use iced::window;
use iced::{Size, Theme};

fn theme(_state: &ClipboardState) -> Theme {
    Theme::custom(
        "Catppuccin Mocha Red".to_string(),
        Palette {
            background: iced::Color::from_rgb8(30, 30, 46),
            text: iced::Color::from_rgb8(205, 214, 244),
            primary: iced::Color::from_rgb8(243, 139, 168),
            success: iced::Color::from_rgb8(166, 227, 161),
            warning: iced::Color::from_rgb8(250, 179, 135),
            danger: iced::Color::from_rgb8(243, 139, 168),
        },
    );
    Theme::SolarizedDark
}

pub fn main() -> iced::Result {
    iced::application(ClipboardState::build, update, view)
        .title(APP_ID)
        .theme(theme)
        .antialiasing(true)
        .window(window::Settings {
            size: Size::new(1024.0, 768.0),
            min_size: Some(Size::new(500.0, 300.0)),
            level: window::Level::AlwaysOnTop,
            position: window::Position::Centered,
            decorations: true,
            ..window::Settings::default()
        })
        .run()
}
