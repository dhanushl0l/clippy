use super::state::ClipboardState;
use super::update::Message;
use iced::Alignment::Center;
use iced::widget::{column, container, scrollable};
use iced::{Element, Length};

pub fn view(clipboard: &ClipboardState) -> Element<'_, Message> {
    container(
        scrollable(
            column(
                clipboard
                    .changed
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
    .align_x(Center)
    .into()
}
