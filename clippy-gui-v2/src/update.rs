use std::path::PathBuf;

use crate::state::ClipboardState;

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
}

pub fn update(clipboard: &mut ClipboardState, message: Message) {
    match message {
        _ => {}
    }
}
