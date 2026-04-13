use std::path::PathBuf;

use crate::{footer::FooterButton, state::ClipboardState};

#[derive(Debug, Clone)]
pub enum ClipboardAction {
    Pin(PathBuf),
    Edit(PathBuf),
    Delete(PathBuf),
    Click(PathBuf),
    None,
}
#[derive(Debug, Clone)]
pub enum Message {
    Clipboard(ClipboardAction),
    FooterAction(FooterButton),
}

pub fn update(clipboard: &mut ClipboardState, message: Message) {
    match message {
        _ => {}
    }
}
