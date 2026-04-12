use crate::UIData;
use crate::update::Message;
use iced::Alignment::Center;
use iced::Length::Fill;
use iced::widget::{button, column, container, row, rule, scrollable, svg, text};
use iced::{Border, Element, Length, padding};
use std::path::PathBuf;

fn action_button<'a, Message>(
    content: impl Into<Element<'a, Message>>,
    on_press: Message,
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
    .width(20)
    .height(25)
    .padding(3)
    .style(|theme: &iced::Theme, status| {
        let mut style = button::primary(theme, status);
        style.background = None;
        style.text_color = theme.palette().text;
        style.shadow = iced::Shadow::default();
        style
    })
    .on_press(on_press)
}

impl UIData {
    pub fn new(data: String, typ: String, device: String, path: PathBuf, pined: bool) -> Self {
        Self {
            data,
            typ,
            device,
            path,
            pined,
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

        container(
            row![
                column![
                    button(
                        container(text(self.data.as_str()))
                            .clip(true)
                            .width(Length::Fill)
                            .height(Length::Fill)
                    )
                    .width(Length::Fill)
                    .height(90)
                    .on_press(Message::Open(id))
                    .style(|theme: &iced::Theme, status| {
                        let mut style = button::primary(theme, status);
                        style.background = None;
                        style.text_color = theme.palette().text;
                        style.shadow = iced::Shadow::default();
                        style
                    }),
                    rule::horizontal(1),
                    container(text(format!("Copied from {}", self.device)).style(
                        |theme: &iced::Theme| text::Style {
                            color: Some(theme.palette().primary),
                        }
                    ))
                    .width(Length::Fill)
                    .padding(3),
                ]
                .width(Length::Fill)
                .spacing(5),
                rule::vertical(1),
                column![
                    action_button(pin_icon, Message::Pin(id)),
                    rule::horizontal(1),
                    action_button(trash, Message::Delete(id)),
                    rule::horizontal(1),
                    action_button(edit, Message::Edit(id)),
                    rule::horizontal(1),
                    action_button(sync, Message::Sync(id)),
                ]
                .width(50)
                .spacing(1)
                .align_x(Center)
                .padding(padding::left(5).right(5)),
            ]
            .padding(5),
        )
        .width(Length::Fill)
        .max_width(700)
        .style(|theme: &iced::Theme| {
            let mut style = container::Style::default();
            style.border = Border {
                color: theme.palette().text,
                width: 1.0,
                radius: 5.0.into(),
            };
            style
        })
        .into()
    }
}
