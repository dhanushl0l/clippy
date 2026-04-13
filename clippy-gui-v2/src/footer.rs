use super::update::Message;
use iced::widget::{button, container, row, svg, text};
use iced::{Border, Element, Length, Padding, padding};

#[derive(Debug, Clone)]
pub enum FooterButton {
    Clipboard,
    Notification,
    Pined,
    Next,
}

impl From<FooterButton> for Message {
    fn from(btn: FooterButton) -> Self {
        Message::FooterAction(btn)
    }
}

fn glass_button(
    icon_bytes: &'static [u8],
    label: &'static str,
    is_active: bool,
    on_press: Message,
) -> Element<'static, Message> {
    let icon = svg(svg::Handle::from_memory(icon_bytes))
        .width(26)
        .height(26)
        .style(move |theme: &iced::Theme, _status| svg::Style {
            color: if is_active {
                Some(theme.extended_palette().primary.base.color)
            } else {
                Some(theme.extended_palette().background.strong.color)
            },
        });

    let col = iced::widget::column![
        icon,
        text(label)
            .size(11)
            .style(move |theme: &iced::Theme| text::Style {
                color: if is_active {
                    Some(theme.extended_palette().primary.base.color)
                } else {
                    Some(theme.extended_palette().background.strong.color)
                },
            })
    ]
    .align_x(iced::Alignment::Center)
    .spacing(5)
    .padding(padding::top(5))
    .width(64);

    button(col)
        .padding(padding::top(4).bottom(8).left(4).right(4))
        .on_press(on_press)
        .style(move |theme: &iced::Theme, status| {
            let palette = theme.extended_palette();
            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => {
                    Some(iced::Background::Color(iced::Color {
                        r: palette.background.strong.color.r,
                        g: palette.background.strong.color.g,
                        b: palette.background.strong.color.b,
                        a: 0.10,
                    }))
                }
                _ => None,
            };

            button::Style {
                background: bg,
                border: Border {
                    radius: 12.0.into(),
                    width: 0.0,
                    color: iced::Color::TRANSPARENT,
                },
                text_color: palette.background.strong.color,
                shadow: iced::Shadow::default(),
                snap: false,
            }
        })
        .into()
}

pub fn footer() -> Element<'static, Message> {
    let clipboard: &'static [u8] = include_bytes!("../../assets/gui_icons/clipboard.svg");
    let notification: &'static [u8] = include_bytes!("../../assets/gui_icons/notification.svg");
    let phone: &'static [u8] = include_bytes!("../../assets/gui_icons/phone.svg");
    let files: &'static [u8] = include_bytes!("../../assets/gui_icons/files.svg");

    let buttons = row![
        glass_button(
            clipboard,
            "Clipboard",
            true,
            Message::FooterAction(FooterButton::Clipboard)
        ),
        glass_button(
            notification,
            "Notification",
            false,
            Message::FooterAction(FooterButton::Notification)
        ),
        glass_button(
            phone,
            "Phone",
            false,
            Message::FooterAction(FooterButton::Pined)
        ),
        glass_button(
            files,
            "Files",
            false,
            Message::FooterAction(FooterButton::Next)
        ),
    ]
    .spacing(0)
    .align_y(iced::Alignment::Center);

    let bar = container(buttons).style(|theme: &iced::Theme| {
        let bg = theme.extended_palette().background.base.color;
        container::Style {
            background: Some(iced::Background::Color(bg)),
            border: Border {
                color: theme.extended_palette().background.strong.color,
                width: 0.0,
                radius: 10.0.into(),
            },
            shadow: iced::Shadow {
                color: iced::Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.35,
                },
                offset: iced::Vector { x: 0.0, y: 6.0 },
                blur_radius: 24.0,
            },
            ..container::Style::default()
        }
    });

    container(bar)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::Alignment::Center)
        .align_y(iced::Alignment::End)
        .padding(padding::bottom(16))
        .into()
}
