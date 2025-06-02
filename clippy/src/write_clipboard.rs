use crate::{Data, set_global_bool};
use base64::{Engine, engine::general_purpose};
use clipboard_rs::{Clipboard, ClipboardContext, RustImageData, common::RustImage};
use std::error::Error;

#[cfg(target_os = "linux")]
pub fn copy_to_linux(data: Data) -> Result<(), String> {
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        copy_to_clipboard_wl(data)
    } else if std::env::var("DISPLAY").is_ok() {
        copy_to_clipboard(data).map_err(|err| format!("{}", err))
    } else {
        Err(format!("No display server detected"))
    }
}

#[cfg(target_os = "linux")]
fn copy_to_clipboard_wl(data: Data) -> Result<(), String> {
    set_global_bool(true);
    push_to_clipboard_wl(data, false)
}

#[cfg(target_os = "linux")]
pub fn push_to_clipboard_wl(data: Data, forground: bool) -> Result<(), String> {
    use base64::{Engine, engine::general_purpose};
    use wl_clipboard_rs::copy::{ClipboardType, MimeType, Options, Source};

    let mut opts = Options::new();
    opts.clipboard(ClipboardType::Regular);
    opts.foreground(forground);

    Ok(if data.typ.starts_with("image/") {
        let data = general_purpose::STANDARD.decode(data.data).unwrap();
        opts.copy(
            Source::Bytes(data.into_boxed_slice()),
            MimeType::Specific("image/png".to_string()),
        )
        .map_err(|err| format!("{}", err))?
    } else {
        opts.copy(
            Source::Bytes(data.data.into_bytes().into_boxed_slice()),
            MimeType::Text,
        )
        .map_err(|err| format!("{}", err))?
    })
}

#[cfg(target_os = "linux")]
pub fn push_to_clipboard_wl_command(data: Data) -> Result<(), String> {
    use base64::{Engine, engine::general_purpose};
    use std::io::Write;
    use std::process::Command;

    let data_u8 = if data.typ.starts_with("image/") {
        general_purpose::STANDARD.decode(&data.data).unwrap()
    } else {
        println!("{}", data.typ);
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

pub fn copy_to_clipboard(data: Data) -> Result<(), Box<dyn Error + Send + Sync>> {
    set_global_bool(true);
    push_to_clipboard(data)
}

pub fn push_to_clipboard(data: Data) -> Result<(), Box<dyn Error + Send + Sync>> {
    let ctx = ClipboardContext::new()?;

    if data.typ.starts_with("image/") {
        ctx.set_image(RustImageData::from_bytes(&string_to_vecu8(data.data))?)?;
    } else {
        ctx.set_text(data.data)?;
    }

    Ok(())
}

// fn read_data(file: String) -> Data {
//     let target = get_path().join(file);
//     let mut file = File::open(target).unwrap();

//     let mut contents = String::new();
//     file.read_to_string(&mut contents).unwrap_or(0);

//     let data: Data = serde_json::from_str(&contents).unwrap_or(Data::new(
//         String::new(),
//         "empty".to_string(),
//         "os".to_string(),
//         true,
//     ));

//     data
// }

pub fn string_to_vecu8(data: String) -> Vec<u8> {
    general_purpose::STANDARD.decode(data).unwrap()
}
