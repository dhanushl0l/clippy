use crate::{Data, get_global_bool, set_global_bool};
use crate::{MessageChannel, UserSettings};
use base64::{Engine, engine::general_purpose};
use chrono::Utc;
use clipboard_rs::common::RustImage;
use clipboard_rs::{Clipboard, ClipboardContext, ClipboardHandler};
use image::{ImageFormat, ImageReader, imageops};
use log::{debug, error};
use std::error;
use std::io::Cursor;
use tokio::sync::mpsc::Sender;

#[cfg(target_os = "linux")]
pub fn read_wayland_clipboard(tx: &Sender<MessageChannel>) -> Result<(), std::io::Error> {
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

    let mut stream = WlClipboardPasteStream::init(WlListenType::ListenOnCopy).unwrap();
    stream.set_priority(preferred_formats);
    for i in stream.paste_stream().flatten().flatten() {
        if get_global_bool() {
            if let Err(e) = parse_wayland_clipboard(i.context, tx) {
                error!("Unable read clipboard: {}", e);
            };
        } else {
            set_global_bool(true);
        }
    }
    Ok(())
}

pub struct Manager<'a> {
    ctx: ClipboardContext,
    tx: &'a Sender<MessageChannel>,
}

impl<'a> Manager<'a> {
    pub fn new(tx: &'a Sender<MessageChannel>) -> Self {
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
            set_global_bool(true);
        }
    }
}

pub fn write_to_json(data: Vec<u8>, typ: String, device: String, tx: &Sender<MessageChannel>) {
    let time = Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string();

    let store_image = match UserSettings::build_user() {
        Ok(settings) => settings.store_image,
        Err(_) => true,
    };

    if typ.starts_with("image/") && store_image {
        use crate::save_image;

        if let Err(e) = save_image(&time, &data) {
            error!("Unable to write thumbnail");
            debug!("{e}")
        };
    }

    let data = if data.len() > 15700268 {
        if typ.starts_with("image/") {
            let data = compress_image(&data).unwrap();
            compress_str(data).unwrap()
        } else {
            String::from_utf8(data[..15700268].to_vec()).unwrap()
        }
    } else {
        if typ.starts_with("image/") {
            compress_str(data).unwrap()
        } else {
            String::from_utf8(data).unwrap()
        }
    };

    let data = Data::new(data, typ, device, false);
    match data.write_to_json(tx, time) {
        Ok(_) => (),
        Err(err) => error!("Unable to write to json: {}", err),
    }
}

#[cfg(target_os = "linux")]
pub fn parse_wayland_clipboard(
    data: wayland_clipboard_listener::ClipBoardListenContext,
    tx: &Sender<MessageChannel>,
) -> Result<(), Box<dyn error::Error>> {
    use crate::UserSettings;
    use chrono::Utc;

    let (typ, data) = (data.mime_type, data.context);
    log::info!("Clipboard data stored: {}", typ);
    let time = Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string();

    let store_image = match UserSettings::build_user() {
        Ok(settings) => settings.store_image,
        Err(_) => true,
    };

    if typ.starts_with("image/") && store_image {
        use crate::save_image;

        if let Err(e) = save_image(&time, &data) {
            error!("Unable to write thumbnail");
            debug!("{e}")
        };
    }

    let json_data = if data.len() > 15700268 {
        if !typ.starts_with("image/") {
            String::from_utf8(data[..15700268].to_vec()).unwrap_or("".to_string())
        } else {
            let data = compress_image(&data)?;
            compress_str(data)?
        }
    } else {
        if !typ.starts_with("image/") {
            String::from_utf8(data).unwrap_or("".to_string())
        } else {
            compress_str(data)?
        }
    };

    let result = Data::new(json_data, typ, "os".to_owned(), false);
    match result.write_to_json(tx, time) {
        Ok(_) => (),
        Err(err) => error!("Unable to write to json: {}", err),
    }
    Ok(())
}

fn compress_str(data: Vec<u8>) -> Result<String, Box<dyn error::Error>> {
    let data = general_purpose::STANDARD.encode(data);
    Ok(data)
}

fn compress_image(img: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    debug!("Starting image compression");
    const MAX_SIZE: usize = 15_700_268;

    let decoded = ImageReader::new(Cursor::new(img))
        .with_guessed_format()?
        .decode()?;

    let original_width = decoded.width();
    let original_height = decoded.height();

    let scale = (MAX_SIZE as f64 / img.len() as f64).sqrt().min(1.0);

    let new_width = (original_width as f64 * scale).round() as u32;
    let new_height = (original_height as f64 * scale).round() as u32;

    let resized = decoded.resize(new_width, new_height, imageops::Triangle);

    let mut out = Vec::new();
    resized.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)?;

    Ok(out)
}
