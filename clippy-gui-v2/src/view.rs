use crate::footer::footer;
use crate::header::header;

use super::state::ClipboardState;
use super::update::Message;
use iced::Alignment::Center;
use iced::widget::{column, container, scrollable};
use iced::{Element, Length};

pub fn view(clipboard_state: &ClipboardState) -> Element<'_, Message> {
    column![
        header(),
        container(
            scrollable(
                column(
                    clipboard_state
                        .clipboard_data
                        .iter()
                        .enumerate()
                        .map(|(i, card)| card.view(i)),
                )
                .spacing(10)
                .padding(10)
                .align_x(Center),
            )
            .direction(scrollable::Direction::Vertical(
                scrollable::Scrollbar::default(),
            )),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Center),
        footer(),
    ]
    .into()
}
