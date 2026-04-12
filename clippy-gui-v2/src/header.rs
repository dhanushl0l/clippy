use super::update::Message;
use iced::widget::{container, row, text};
use iced::{Border, Element, Length, padding};

pub fn header() -> Element<'static, Message> {
    container(
        row![
            text("Clippy").size(20),
            text("v0.1.0").style(|theme: &iced::Theme| text::Style {
                color: Some(theme.extended_palette().background.strong.color),
            }),
        ]
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
