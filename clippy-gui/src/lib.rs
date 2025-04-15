#[cfg(target_os = "linux")]
pub fn copy_to_linux(typ: String, data: String) {
    use clippy::write_clipboard::{
        push_to_clipboard, push_to_clipboard_wl, push_to_clipboard_wl_command,
    };

    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        // push_to_clipboard_wl(typ, data, true);
        push_to_clipboard_wl_command(typ, data);
    } else if std::env::var("DISPLAY").is_ok() {
        push_to_clipboard(typ, data);
    }
}
