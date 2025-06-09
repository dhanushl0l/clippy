use std::sync::Arc;

use crate::{Data, Pending, get_global_bool, set_global_bool};
use base64::{Engine, engine::general_purpose};
use clipboard_rs::common::RustImage;
use clipboard_rs::{Clipboard, ClipboardContext, ClipboardHandler};
use log::{debug, error, info};
use tokio::sync::mpsc::Sender;

#[cfg(target_os = "linux")]
pub fn read_wayland_clipboard(pending: Arc<Pending>) -> Result<(), wl_clipboard_rs::paste::Error> {
    use std::collections::HashSet;
    use std::io::Read;
    use wl_clipboard_rs::paste::{ClipboardType, MimeType, Seat, get_contents, get_mime_types};

    if get_global_bool() {
        let typ: HashSet<String> = get_mime_types(ClipboardType::Regular, Seat::Unspecified)?;

        for i in &typ {
            if i == "text/clippy" {
                return Ok(());
            }
        }

        let preferred_formats = [
            "image/png",
            "image/jpeg",
            "image/jxl",
            "image/tiff",
            "image/bmp",
            "text/plain;charset=utf-8",
            "text/plain",
            "STRING",
            "UTF8_STRING",
            "text/uri-list",
        ];

        let mut main_type = String::new();
        // Check for preferred formats in order of priority
        for &format in &preferred_formats {
            if let Some(fallback) = typ.iter().find(|m| *m == format) {
                main_type = fallback.to_owned();
                break;
            }
        }

        let mime_type = MimeType::Specific(&main_type);
        let (mut dat, typ) = get_contents(ClipboardType::Regular, Seat::Unspecified, mime_type)?;
        let mut vec = Vec::new();
        let _ = dat.read_to_end(&mut vec);

        parse_wayland_clipboard(typ, vec, pending);
    } else {
        set_global_bool(false);
    }
    Ok(())
}

pub struct Manager {
    ctx: ClipboardContext,
    pending: Arc<Pending>,
}

impl Manager {
    pub fn new(pending: Arc<Pending>) -> Self {
        let ctx = ClipboardContext::new().unwrap();
        Manager { ctx, pending }
    }
}

impl<'a> ClipboardHandler for Manager {
    fn on_clipboard_change(&mut self) {
        if get_global_bool() {
            let ctx = &self.ctx;
            let types = ctx.available_formats().unwrap();

            debug!("Available types: {:?}", types);

            if let Ok(val) = ctx.get_image() {
                debug!("Type img");
                write_to_json(
                    val.to_png().unwrap().get_bytes().to_vec(),
                    String::from("image/png"),
                    String::from("os"),
                    self.pending.clone(),
                );
            } else if let Ok(val) = ctx.get_text() {
                write_to_json(
                    val.into_bytes(),
                    String::from("String"),
                    String::from("os"),
                    self.pending.clone(),
                );
            }
        } else {
            set_global_bool(false);
        }
    }
}

pub fn write_to_json(data: Vec<u8>, typ: String, device: String, pending: Arc<Pending>) {
    let data = if typ.starts_with("image/") {
        compress_str(data).unwrap()
    } else {
        String::from_utf8(data).unwrap()
    };

    let data = Data::new(data, typ, device, false);
    match data.write_to_json(pending) {
        Ok(_) => (),
        Err(err) => error!("Unable to write to json: {}", err),
    }
}

pub fn parse_wayland_clipboard(typ: String, data: Vec<u8>, pending: Arc<Pending>) {
    info!("Clipboard data stored: {}", typ);

    let json_data;
    if !typ.starts_with("image/") {
        json_data = String::from_utf8(data).unwrap_or("".to_string());
    } else {
        json_data = compress_str(data).unwrap();
    }

    let result = Data::new(json_data, typ, "os".to_owned(), false);
    match result.write_to_json(pending) {
        Ok(_) => (),
        Err(err) => error!("Unable to write to json: {}", err),
    }
}

fn compress_str(data: Vec<u8>) -> Result<String, ()> {
    Ok(general_purpose::STANDARD.encode(data))
}
