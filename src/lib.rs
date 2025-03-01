use serde::{Deserialize, Serialize};
use std::io::Write;
use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Read},
    path::PathBuf,
};
use wl_clipboard_rs::paste::{ClipboardType, MimeType, Seat, get_contents};

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
        let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let config_path: PathBuf = [home.as_str(), ".local/share/clippy/data/data.json"]
            .iter()
            .collect();

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json_data = serde_json::to_string(&self).expect("Failed to serialize data");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(config_path)?;

        writeln!(file, "{}", json_data)?;
        Ok(())
    }
}
