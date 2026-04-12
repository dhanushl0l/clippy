use crate::update::{ClipboardAction, Message};
use crate::{MetaData, UIData};
use iced::Alignment::Center;
use iced::widget::image;
use iced::widget::image::Handle;
use iced::widget::{button, column, container, row, rule, svg, text};
use iced::{Border, Color, Element, Length, padding};
use std::path::PathBuf;

fn action_button<'a, Message>(
    content: impl Into<Element<'a, Message>>,
    on_press: Message,
    pined: bool,
) -> iced::widget::Button<'a, Message>
where
    Message: Clone + 'a,
{
    button(
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Center)
            .align_y(iced::alignment::Vertical::Center),
    )
    .width(30)
    .height(25)
    .padding(5)
    .style(move |theme: &iced::Theme, status| {
        let mut style = button::primary(theme, status);
        style.shadow = iced::Shadow::default();
        style.text_color = theme.palette().text;
        let base_color = theme.palette().text;

        if pined {
            style.background = Some(
                Color {
                    a: 0.15,
                    ..theme.palette().primary
                }
                .into(),
            );
        } else {
            style.border = Border::default();
            style.background = None;
        }

        match status {
            button::Status::Hovered => {
                style.background = Some(
                    Color {
                        a: 0.10,
                        ..base_color
                    }
                    .into(),
                );
            }
            button::Status::Pressed => {
                style.background = Some(
                    Color {
                        a: 0.20,
                        ..base_color
                    }
                    .into(),
                );
            }
            _ => {}
        }

        style
    })
    .on_press(on_press)
}

impl UIData {
    pub fn new(
        data: String,
        typ: String,
        device: String,
        path: PathBuf,
        pined: bool,
        sync: bool,
    ) -> Self {
        Self {
            data: MetaData::Text(data),
            typ,
            device,
            path,
            pined,
            sync,
        }
    }

    pub fn view(&self, id: usize) -> Element<Message> {
        let pin_icon = svg(svg::Handle::from_memory(
            include_bytes!("../../assets/gui_icons/pin.svg").as_ref(),
        ))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|theme: &iced::Theme, _status| svg::Style {
            color: Some(theme.palette().text),
        });
        let trash = svg(svg::Handle::from_memory(
            include_bytes!("../../assets/gui_icons/trash.svg").as_ref(),
        ))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|theme: &iced::Theme, _status| svg::Style {
            color: Some(theme.palette().text),
        });
        let edit = svg(svg::Handle::from_memory(
            include_bytes!("../../assets/gui_icons/edit.svg").as_ref(),
        ))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|theme: &iced::Theme, _status| svg::Style {
            color: Some(theme.palette().text),
        });

        let sync = svg(svg::Handle::from_memory(
            include_bytes!("../../assets/gui_icons/sync.svg").as_ref(),
        ))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|theme: &iced::Theme, _status| svg::Style {
            color: Some(theme.palette().text),
        });

        let mut actions: Vec<Element<Message>> = vec![
            action_button(
                pin_icon,
                Message::Clipboard(ClipboardAction::Pin(self.path.clone())),
                self.pined,
            )
            .into(),
            rule::horizontal(1).into(),
            action_button(
                trash,
                Message::Clipboard(ClipboardAction::Delete(self.path.clone())),
                false,
            )
            .into(),
            rule::horizontal(1).into(),
            action_button(
                edit,
                Message::Clipboard(ClipboardAction::Edit(self.path.clone())),
                false,
            )
            .into(),
        ];

        if !self.sync {
            actions.push(rule::horizontal(1).into());
            actions
                .push(action_button(sync, Message::Clipboard(ClipboardAction::None), false).into());
        }

        let action_column = column(actions)
            .width(Length::Shrink)
            .height(Length::Shrink)
            .align_x(Center);

        container(row![
            column![
                button(
                    container(match &self.data {
                        MetaData::Text(text) => {
                            Element::from(iced::widget::text(text))
                        }
                        MetaData::Image(width, height, bytes) => {
                            Element::from(container(
                                image(Handle::from_rgba(*width, *height, bytes.clone()))
                                    .height(Length::Fill),
                            ))
                        }
                    })
                    .clip(true)
                    .width(Length::Fill)
                    .height(Length::Fill)
                )
                .width(Length::Fill)
                .height(90)
                .on_press(Message::Clipboard(ClipboardAction::Click(
                    self.path.clone()
                )))
                .style(|theme: &iced::Theme, status| {
                    let mut style = button::primary(theme, status);
                    style.background = None;
                    style.text_color = theme.palette().text;
                    style.shadow = iced::Shadow::default();
                    let base_color = theme.palette().text;
                    match status {
                        button::Status::Hovered => {
                            style.background = Some(
                                Color {
                                    a: 0.10,
                                    ..base_color
                                }
                                .into(),
                            );
                        }
                        button::Status::Pressed => {
                            style.background = Some(
                                Color {
                                    a: 0.20,
                                    ..base_color
                                }
                                .into(),
                            );
                        }
                        _ => {
                            style.background = None;
                        }
                    }
                    style
                }),
                rule::horizontal(1),
                container(
                    row![
                        text(format!("from: {}", self.device)).style(|theme: &iced::Theme| {
                            text::Style {
                                color: Some(theme.palette().primary),
                            }
                        }),
                        rule::vertical(1),
                        text(format!("type: {}", self.typ)).style(|theme: &iced::Theme| {
                            text::Style {
                                color: Some(theme.palette().primary),
                            }
                        }),
                    ]
                    .spacing(10)
                    .align_y(iced::Alignment::Center)
                )
                .width(Length::Fill)
                .clip(true)
                .padding(padding::left(5).top(4).bottom(4)),
            ]
            .width(Length::Fill)
            .spacing(0),
            rule::vertical(1),
            action_column
        ])
        .width(Length::Fill)
        .max_width(700)
        .height(130) // fixed height keeps everything inside the border
        .style(|theme: &iced::Theme| {
            let mut style = container::Style::default();
            style.border = Border {
                color: theme.extended_palette().background.strong.color,
                width: 1.0,
                radius: 5.0.into(),
            };
            style
        })
        .into()
    }
}
