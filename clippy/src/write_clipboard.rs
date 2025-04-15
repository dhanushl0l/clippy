use crate::{Data, UserData, get_path};
use base64::{Engine, engine::general_purpose};
use clipboard_rs::{Clipboard, ClipboardContext, RustImageData, common::RustImage};
use std::{
    error::Error,
    fs::File,
    io::{Read, Write},
    process::{Command, Stdio},
};

#[cfg(target_os = "linux")]
pub fn copy_to_linux(userdata: &UserData) -> Result<(), String> {
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        copy_to_clipboard_wl(userdata)
    } else if std::env::var("DISPLAY").is_ok() {
        copy_to_clipboard(userdata).map_err(|err| format!("{}", err))
    } else {
        Err(format!("No display server detected"))
    }
}

#[cfg(target_os = "linux")]
fn copy_to_clipboard_wl(userdata: &UserData) -> Result<(), String> {
    use crate::set_global_bool;

    let data = read_data(userdata.last_one());
    set_global_bool(false);

    push_to_clipboard_wl(data.typ, data.data, false)
}

#[cfg(target_os = "linux")]

pub fn push_to_clipboard_wl(typ: String, data: String, forground: bool) -> Result<(), String> {
    use base64::{Engine, engine::general_purpose};
    use wl_clipboard_rs::copy::{ClipboardType, MimeType, Options, Source};

    let mut opts = Options::new();
    opts.clipboard(ClipboardType::Regular);
    opts.foreground(forground);

    Ok(if typ.starts_with("image/") {
        let data = general_purpose::STANDARD.decode(data).unwrap();
        opts.copy(
            Source::Bytes(data.into_boxed_slice()),
            MimeType::Specific("image/png".to_string()),
        )
        .map_err(|err| format!("{}", err))?
    } else {
        opts.copy(
            Source::Bytes(data.into_bytes().into_boxed_slice()),
            MimeType::Text,
        )
        .map_err(|err| format!("{}", err))?
    })
}

#[cfg(target_os = "linux")]
pub fn push_to_clipboard_wl_command(typ: String, data: String) -> Result<(), String> {
    use base64::{Engine, engine::general_purpose};
    let data = if typ.starts_with("image/") {
        general_purpose::STANDARD.decode(data).unwrap()
    } else {
        println!("{}", typ);
        data.into_bytes()
    };

    let mut cmd = Command::new("wl-copy")
        .arg("--type")
        .arg(typ)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start wl-copy: {}", e))?;

    if let Some(stdin) = cmd.stdin.as_mut() {
        stdin
            .write_all(&data)
            .map_err(|e| format!("Failed to write to wl-copy: {}", e))?;
    }

    // Detach — we don’t wait on the process
    Ok(())
}

pub fn copy_to_clipboard(userdata: &UserData) -> Result<(), Box<dyn Error + Send + Sync>> {
    use crate::set_global_bool;
    let data = read_data(userdata.last_one());

    set_global_bool(false);

    let typ = data.typ;
    let data = data.data;

    push_to_clipboard(typ, data)
}

pub fn push_to_clipboard(typ: String, data: String) -> Result<(), Box<dyn Error + Send + Sync>> {
    let ctx = ClipboardContext::new()?;

    if typ.starts_with("image/") {
        ctx.set_image(RustImageData::from_bytes(&string_to_vecu8(data))?)?;
    } else {
        ctx.set_text(data)?;
    }

    Ok(())
}

fn read_data(file: String) -> Data {
    let target = get_path().join(file);
    let mut file = File::open(target).unwrap();

    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap_or(0);

    let data: Data = serde_json::from_str(&contents).unwrap_or(Data::new(
        String::new(),
        "empty".to_string(),
        "os".to_string(),
        true,
    ));

    data
}

pub fn string_to_vecu8(data: String) -> Vec<u8> {
    general_purpose::STANDARD.decode(data).unwrap()
}
