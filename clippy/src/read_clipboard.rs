use crate::{Data, get_global_bool, set_global_bool};
use chrono::prelude::Utc;
use clipboard_rs::common::RustImage;
use clipboard_rs::{Clipboard, ClipboardContext, ClipboardHandler, RustImageData};
use std::collections::HashSet;
use std::io::{Read, Write};
use std::sync::mpsc::Sender;
use std::{
    env,
    fs::File,
    fs::{self},
    io::{self},
};

#[cfg(target_os = "linux")]
use wl_clipboard_rs::paste::{ClipboardType, Error, MimeType, Seat, get_contents, get_mime_types};

#[cfg(target_os = "linux")]
pub fn read_wayland_clipboard(tx: &Sender<(String, String)>) -> Result<(), Error> {
    use crate::{get_global_bool, set_global_bool};

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

        parse_wayland_clipboard(typ, vec, tx);
    } else {
        set_global_bool(true);
    }
    Ok(())
}

pub struct Manager<'a> {
    ctx: ClipboardContext,
    tx: &'a Sender<(String, String)>,
}

impl<'a> Manager<'a> {
    pub fn new(tx: &'a Sender<(String, String)>) -> Self {
        let ctx = ClipboardContext::new().unwrap();
        Manager { ctx, tx }
    }
}

impl<'a> ClipboardHandler for Manager<'a> {
    fn on_clipboard_change(&mut self) {
        if get_global_bool() {
            let ctx = &self.ctx;
            let types = ctx.available_formats().unwrap();

            eprintln!("{:?}", types);

            if let Ok(val) = ctx.get_image() {
                match write_img_json(val, String::from("os")) {
                    Ok(_) => (),
                    Err(err) => eprintln!("{:?}", err),
                }
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

pub fn write_to_json(data: Vec<u8>, typ: String, device: String, tx: &Sender<(String, String)>) {
    let data = Data::new(data, typ, device, false);
    match data.write_to_json(tx) {
        Ok(_) => (),
        Err(err) => eprintln!("{}", err),
    }
}

fn write_img_json(img: RustImageData, os: String) -> Result<(), io::Error> {
    let time = Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string();

    let path = crate::get_path().join(format!("{}", time));

    fs::create_dir_all(path.to_str().unwrap())?;

    let image = img.to_png().expect("msg");

    let json_data = Data::new(image.get_bytes().to_vec(), "IMG".to_string(), os, false);
    let json_data = serde_json::to_string_pretty(&json_data)?;

    let img_path = path.join("img.png");
    let img_path_str = img_path
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Invalid UTF-8 in path"))?;

    image
        .save_to_path(img_path_str)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to save image: {e}")))?;

    let mut file = File::create(&path.join("data.json"))?;
    file.write_all(json_data.as_bytes())?;

    Ok(())
}

pub fn parse_wayland_clipboard(typ: String, data: Vec<u8>, tx: &Sender<(String, String)>) {
    println!("{}", typ);

    let result = Data::new(data, typ, "os".to_owned(), false);
    match result.write_to_json(tx) {
        Ok(_) => (),
        Err(err) => eprintln!("{:?}", err),
    }
}
