use crate::state::ClipboardState;

#[derive(Debug, Clone)]
pub enum Message {
    Open(usize),
    Pin(usize),
    Edit(usize),
    Delete(usize),
    Sync(usize),
}

pub fn update(clipboard: &mut ClipboardState, message: Message) {
    match message {
        _ => {}
    }
}
