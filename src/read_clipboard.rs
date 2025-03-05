use chrono::prelude::Utc;
use clipboard_rs::common::RustImage;
use clipboard_rs::{Clipboard, ClipboardContext, ClipboardHandler, RustImageData};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::process;
use std::{
    env,
    fs::{self},
    io::{self, Read},
    path::PathBuf,
};
use wl_clipboard_rs::paste::{ClipboardType, MimeType, Seat, get_contents};

static PATH: &str = env::consts::OS;

pub fn read_wayland_clipboard() -> Result<(Vec<u8>, String), ()> {
    match get_contents(ClipboardType::Regular, Seat::Unspecified, MimeType::Any) {
        Ok((mut pipe, mime_type)) => {
            // println!("{:?}", mime_type);

            let mut contents = Vec::new();
            if let Err(e) = pipe.read_to_end(&mut contents) {
                eprintln!("Failed to read clipboard data: {}", e);
            }

            match mime_type.as_str() {
                "text/plain;charset=utf-8" => Ok((contents, mime_type)),
                "UTF8_STRING" => Ok((contents, mime_type)),
                "STRING" => Ok((contents, mime_type)),
                "text/html" => Ok((contents, mime_type)),
                "text/uri-list" => Ok((contents, mime_type)),
                "image/png" => Ok((contents, mime_type)),
                "image/jpeg" => Ok((contents, mime_type)),
                _ => Err(()),
            }
        }
        Err(_) => Err(()),
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Data {
    data: Vec<u8>,
    typ: String,
    device: String,
}

impl Data {
    pub fn new(data: Vec<u8>, typ: String, device: String) -> Self {
        Data { data, typ, device }
    }

    pub fn write_to_json(&self) -> Result<(), io::Error> {
        let time = Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string();

        fs::create_dir_all(&get_path(PATH))?;

        let file_path = get_path(PATH).join(format!("{}.json", time));

        let json_data = serde_json::to_string_pretty(self)?;

        let mut file = File::create(file_path)?;
        file.write_all(json_data.as_bytes())?;

        Ok(())
    }
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
    let data = Data::new(data, typ, device);
    match data.write_to_json() {
        Ok(_) => (),
        Err(err) => eprintln!("{}", err),
    }
}

fn write_img_json(img: RustImageData, os: String, file_data: Vec<u8>) -> Result<(), io::Error> {
    let time = Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string();

    fs::create_dir_all(&get_path(PATH))?;

    let json_data = Data::new(file_data, "IMG".to_string(), os);
    let file_path = get_path(PATH).join(format!("{}.json", time));

    let json_data = serde_json::to_string_pretty(&json_data)?;

    let mut file = File::create(file_path)?;
    file.write_all(json_data.as_bytes())?;

    match img.save_to_path(get_path(PATH).to_str().unwrap()) {
        Ok(_) => (),
        Err(err) => eprint!("{:?}", err),
    };

    Ok(())
}

fn get_path(os: &str) -> PathBuf {
    match os {
        "linux" | "mac" => {
            let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            [home.as_str(), ".local/share/clippy/data"].iter().collect()
        }
        "windows" => {
            let home =
                env::var("APPDATA").unwrap_or_else(|_| "C:\\Users\\Public\\AppData".to_string());
            [home.as_str(), "clippy\\data"].iter().collect()
        }

        _ => {
            eprintln!("unsuported hardware");
            process::exit(1)
        }
    }
}
