use super::update::Message;
use clippy::APP_VERSION;
use iced::widget::{button, container, row, svg, text};
use iced::{Border, Color, Element, Length, padding};

fn icon_button(path: &'static [u8]) -> iced::widget::Button<'static, Message> {
    button(
        svg(svg::Handle::from_memory(path))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|theme: &iced::Theme, _status| svg::Style {
                color: Some(theme.palette().text),
            }),
    )
    .on_press(Message::Clipboard(crate::update::ClipboardAction::None))
    .height(25)
    .width(25)
    .padding(4)
    .style(|theme: &iced::Theme, status| {
        let mut style = button::primary(theme, status);
        let base_color = theme.palette().text;
        style.shadow = iced::Shadow::default();
        style.text_color = base_color;
        style.background = match status {
            button::Status::Hovered => Some(
                Color {
                    a: 0.10,
                    ..base_color
                }
                .into(),
            ),
            button::Status::Pressed => Some(
                Color {
                    a: 0.20,
                    ..base_color
                }
                .into(),
            ),
            _ => None,
        };
        style
    })
}

fn icon_button_plain(path: &'static [u8]) -> iced::widget::Button<'static, Message> {
    button(
        svg(svg::Handle::from_memory(path))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|theme: &iced::Theme, _status| svg::Style {
                color: Some(theme.palette().text),
            }),
    )
    .on_press(Message::Clipboard(crate::update::ClipboardAction::None))
    .height(37) // matches left container height
    .width(25)
    .padding(4)
    .style(|theme: &iced::Theme, status| {
        let mut style = button::primary(theme, status);
        let base_color = theme.palette().text;
        style.shadow = iced::Shadow::default();
        style.text_color = base_color;
        style.border = Border::default();
        style.background = match status {
            button::Status::Hovered => Some(
                Color {
                    a: 0.10,
                    ..base_color
                }
                .into(),
            ),
            button::Status::Pressed => Some(
                Color {
                    a: 0.20,
                    ..base_color
                }
                .into(),
            ),
            _ => None,
        };
        style
    })
}

pub fn header() -> Element<'static, Message> {
    container(
        row![
            // Left: outlined box with centered buttons
            container(
                container(
                    row![
                        icon_button(include_bytes!("../../assets/gui_icons/prev.svg").as_ref()),
                        icon_button(include_bytes!("../../assets/gui_icons/next.svg").as_ref()),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center)
                )
                .align_x(iced::Alignment::Center)
                .align_y(iced::Alignment::Center)
                .width(Length::Shrink)
                .height(37)
                .padding(4)
                .style(|theme: &iced::Theme| container::Style {
                    border: Border {
                        color: theme.extended_palette().background.strong.color,
                        width: 1.0,
                        radius: 10.0.into(),
                    },
                    ..container::Style::default()
                }),
            )
            .width(Length::Fill)
            .align_x(iced::Alignment::Start)
            .align_y(iced::Alignment::Center),
            // Center: Clippy + version
            container(
                row![
                    text("Clippy").size(20),
                    text(APP_VERSION).style(|theme: &iced::Theme| text::Style {
                        color: Some(theme.extended_palette().background.strong.color),
                    }),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center)
            )
            .width(Length::Shrink)
            .align_x(iced::Alignment::Center)
            .align_y(iced::Alignment::Center),
            // Right: no outline, buttons fill height of left container
            container(
                row![
                    icon_button_plain(include_bytes!("../../assets/gui_icons/add.svg").as_ref()),
                    icon_button_plain(
                        include_bytes!("../../assets/gui_icons/settings.svg").as_ref()
                    ),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
            )
            .width(Length::Fill)
            .align_x(iced::Alignment::End)
            .align_y(iced::Alignment::Center),
        ]
        .height(Length::Fill)
        .align_y(iced::Alignment::Center)
        .padding(padding::left(15).right(15)),
    )
    .width(Length::Fill)
    .height(50)
    .style(|theme: &iced::Theme| container::Style {
        border: Border {
            color: theme.extended_palette().background.strong.color,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    })
    .into()
}
