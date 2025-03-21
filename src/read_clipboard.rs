use crate::{Data, PATH};
use chrono::prelude::Utc;
use clipboard_rs::common::RustImage;
use clipboard_rs::{Clipboard, ClipboardContext, ClipboardHandler, RustImageData};
use std::io::{Read, Write};
use std::{
    env,
    fs::File,
    fs::{self},
    io::{self},
};

#[cfg(target_os = "linux")]
use wl_clipboard_rs::paste::{ClipboardType, Error, MimeType, Seat, get_contents, get_mime_types};

#[cfg(target_os = "linux")]
pub fn read_wayland_clipboard() -> Result<(), Error> {
    let typ: std::collections::HashSet<String> =
        get_mime_types(ClipboardType::Regular, Seat::Unspecified)?;
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

    parse_wayland_clipboard(typ, vec);

    Ok(())
}

pub struct Manager {
    ctx: ClipboardContext,
}

impl Manager {
    pub fn new() -> Self {
        let ctx = ClipboardContext::new().unwrap();
        Manager { ctx }
    }
}

impl ClipboardHandler for Manager {
    fn on_clipboard_change(&mut self) {
        let ctx = &self.ctx;
        let types = ctx.available_formats().unwrap();

        // debug
        if env::var("DEBUG").is_ok() {
            eprintln!("{:?}", types);

            let content = ctx.get_text().unwrap_or("".to_string());

            println!("txt={}", content);
        }

        if let Ok(val) = ctx.get_image() {
            let data: Vec<u8> = ctx
                .get_text()
                .map(|s| s.as_bytes().to_vec())
                .unwrap_or_default();

            match write_img_json(val, String::from("os"), data) {
                Ok(_) => (),
                Err(err) => eprintln!("{:?}", err),
            }
        } else if let Ok(val) = ctx.get_text() {
            write_to_json(val.into_bytes(), String::from("String"), String::from("os"));
        }
    }
}

pub fn write_to_json(data: Vec<u8>, typ: String, device: String) {
    let data = Data::new(data, typ, device, false);
    match data.write_to_json() {
        Ok(_) => (),
        Err(err) => eprintln!("{}", err),
    }
}

fn write_img_json(img: RustImageData, os: String, file_data: Vec<u8>) -> Result<(), io::Error> {
    let time = Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string();

    let path = crate::get_path(PATH).join(format!("{}", time));

    fs::create_dir_all(path.to_str().unwrap())?;

    let json_data = Data::new(file_data, "IMG".to_string(), os, false);

    let json_data = serde_json::to_string_pretty(&json_data)?;

    match img
        .to_png()
        .expect("error exporting img")
        .save_to_path(path.join("img.png").to_str().expect("error exporting img"))
    {
        Ok(_) => {
            let mut file = File::create(&path.join("data.json"))?;
            file.write_all(json_data.as_bytes())?;
        }
        Err(err) => eprint!("{:?}", err),
    };

    Ok(())
}

pub fn parse_wayland_clipboard(typ: String, data: Vec<u8>) {
    println!("{}", typ);
    match typ.as_str() {
        _ if typ.starts_with("image/") => match save_image(&data, "output.png") {
            Ok(_) => (),
            Err(err) => println!("{:?}", err),
        },
        _ => {
            let result = Data::new(data, typ, "os".to_owned(), false);
            match result.write_to_json() {
                Ok(_) => (),
                Err(err) => eprintln!("{:?}", err),
            }
        }
    }
}

fn save_image(image_data: &[u8], typ: &str) -> Result<(), io::Error> {
    let time = Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string();

    let path = crate::get_path(PATH).join(format!("{}", time));

    fs::create_dir_all(path.to_str().unwrap())?;

    let json_data = Data::new(vec![], typ.to_owned(), "os".to_owned(), false);

    let json_data = serde_json::to_string_pretty(&json_data)?;

    let mut img_file = File::create(path.join("img.png"))?;
    let mut json_file = File::create(path.join("data.json"))?;

    json_file.write_all(json_data.as_bytes())?;
    img_file.write_all(image_data)?;

    Ok(())
}
