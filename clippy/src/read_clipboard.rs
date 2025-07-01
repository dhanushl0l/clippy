#[cfg(target_os = "linux")]
use std::io;

use crate::{Data, get_global_bool, set_global_bool};
use base64::{Engine, engine::general_purpose};
use clipboard_rs::common::RustImage;
use clipboard_rs::{Clipboard, ClipboardContext, ClipboardHandler};
use log::{debug, error};
use tokio::sync::mpsc::Sender;

#[cfg(target_os = "linux")]
pub fn read_wayland_clipboard(tx: &Sender<(String, String, String)>) -> Result<(), io::Error> {
    use wayland_clipboard_listener::{WlClipboardPasteStream, WlListenType};

    let preferred_formats: Vec<String> = [
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
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    set_global_bool(true);

    let mut stream = WlClipboardPasteStream::init(WlListenType::ListenOnCopy).unwrap();
    stream.set_priority(preferred_formats);
    for i in stream.paste_stream().flatten().flatten() {
        if get_global_bool() {
            parse_wayland_clipboard(i.context, tx);
        } else {
            set_global_bool(false);
        }
    }
    Ok(())
}

pub struct Manager<'a> {
    ctx: ClipboardContext,
    tx: &'a Sender<(String, String, String)>,
}

impl<'a> Manager<'a> {
    pub fn new(tx: &'a Sender<(String, String, String)>) -> Self {
        let ctx = ClipboardContext::new().unwrap();
        Manager { ctx, tx }
    }
}

impl<'a> ClipboardHandler for Manager<'a> {
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
                    &self.tx,
                );
            } else if let Ok(val) = ctx.get_text() {
                write_to_json(
                    val.into_bytes(),
                    String::from("String"),
                    String::from("os"),
                    &self.tx,
                );
            }
        } else {
            set_global_bool(false);
        }
    }
}

pub fn write_to_json(
    data: Vec<u8>,
    typ: String,
    device: String,
    tx: &Sender<(String, String, String)>,
) {
    let data = if typ.starts_with("image/") {
        compress_str(data).unwrap()
    } else {
        String::from_utf8(data).unwrap()
    };

    let data = Data::new(data, typ, device, false);
    match data.write_to_json(tx) {
        Ok(_) => (),
        Err(err) => error!("Unable to write to json: {}", err),
    }
}

#[cfg(target_os = "linux")]
pub fn parse_wayland_clipboard(
    data: wayland_clipboard_listener::ClipBoardListenContext,
    tx: &Sender<(String, String, String)>,
) {
    let (typ, data) = (data.mime_type, data.context);
    log::info!("Clipboard data stored: {}", typ);

    let json_data;
    if !typ.starts_with("image/") {
        json_data = String::from_utf8(data).unwrap_or("".to_string());
    } else {
        json_data = compress_str(data).unwrap();
    }

    let result = Data::new(json_data, typ, "os".to_owned(), false);
    match result.write_to_json(tx) {
        Ok(_) => (),
        Err(err) => error!("Unable to write to json: {}", err),
    }
}

fn compress_str(data: Vec<u8>) -> Result<String, ()> {
    Ok(general_purpose::STANDARD.encode(data))
}
