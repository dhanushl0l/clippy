use crate::{Data, set_global_bool};
use base64::{Engine, engine::general_purpose};
use clipboard_rs::{Clipboard, ClipboardContext, RustImageData, common::RustImage};
use std::{error::Error, thread, time::Duration};

#[cfg(target_family = "unix")]
pub fn copy_to_unix(data: Data, paste_on_click: bool) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            return copy_to_clipboard_wl(data, paste_on_click);
        }
    }

    copy_to_clipboard(data, paste_on_click).map_err(|err| format!("{}", err))
}

#[cfg(target_os = "linux")]
pub fn copy_to_clipboard_wl(data: Data, paste_on_click: bool) -> Result<(), String> {
    use wayland_clipboard_listener::WlClipboardCopyStream;
    set_global_bool(false);
    thread::spawn(move || {
        let context = if data.typ.starts_with("text") {
            data.data.into_bytes()
        } else {
            string_to_vecu8(data.data)
        };

        let mut stream = WlClipboardCopyStream::init().unwrap();
        stream
            .copy_to_clipboard(context, vec![&data.typ], false)
            .unwrap();
    });
    #[cfg(feature = "default")]
    if paste_on_click {
        ctrl_v();
    }

    Ok(())
}

#[cfg(target_os = "linux")]
pub fn push_to_clipboard_wl_command(data: Data) -> Result<(), String> {
    use base64::{Engine, engine::general_purpose};
    use std::io::Write;
    use std::process::Command;

    let data_u8 = if data.typ.starts_with("image/") {
        general_purpose::STANDARD.decode(&data.data).unwrap()
    } else {
        data.data.into_bytes()
    };

    let mut cmd = Command::new("wl-copy")
        .arg("--type")
        .arg(data.typ)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start wl-copy: {}", e))?;

    if let Some(stdin) = cmd.stdin.as_mut() {
        stdin
            .write_all(&data_u8)
            .map_err(|e| format!("Failed to write to wl-copy: {}", e))?;
    }

    Ok(())
}

pub fn copy_to_clipboard(
    data: Data,
    paste_on_click: bool,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    set_global_bool(false);
    let ctx = ClipboardContext::new()?;

    if data.typ.starts_with("image/") {
        ctx.set_image(RustImageData::from_bytes(&string_to_vecu8(data.data))?)?;
    } else {
        ctx.set_text(data.data)?;
    }
    #[cfg(feature = "default")]
    if paste_on_click {
        ctrl_v();
    }
    Ok(())
}

#[cfg(feature = "default")]
fn ctrl_v() {
    thread::sleep(Duration::from_millis(500));
    {
        use enigo::{
            Direction::{Click, Press, Release},
            Enigo, Key, Keyboard, Settings,
        };
        let mut enigo = Enigo::new(&Settings::default()).unwrap();

        let key = {
            #[cfg(target_os = "macos")]
            {
                Key::Meta
            }

            #[cfg(not(target_os = "macos"))]
            {
                Key::Control
            }
        };

        enigo.key(key, Press).unwrap();
        enigo.key(Key::Unicode('v'), Click).unwrap();
        enigo.key(key, Release).unwrap();
    }
}

pub fn string_to_vecu8(data: String) -> Vec<u8> {
    general_purpose::STANDARD.decode(data).unwrap()
}
