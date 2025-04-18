pub enum Thumbnail {
    Image((Vec<u8>, (u32, u32))),
    Text(String),
}

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

pub fn str_formate(text: &str) -> String {
    let mut result = String::new();
    let mut count = 0;

    for line in text.lines() {
        if count >= 10 {
            break;
        }

        if line.len() > 100 {
            result.push_str(&line[..100]);
            result.push_str("....\n");
            count += 1;
        } else {
            result.push_str(line);
            result.push('\n');
            count += 1;
        }
    }

    result
}
