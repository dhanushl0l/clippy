pub mod encryption_decryption;
pub mod http;
pub mod read_clipboard;
pub mod user;
pub mod write_clipboard;

use base64::Engine;
use base64::engine::general_purpose;
use bytes::Bytes;
use chrono::prelude::Utc;
use image::ImageReader;
use log::{debug, error, warn};
use serde::{Deserialize, Serialize};
use std::fs::DirEntry;
use std::io::{Cursor, Error, Write};
use std::path::Path;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::{
    collections::BTreeSet,
    env,
    fs::File,
    fs::{self},
    io::{self},
    path::PathBuf,
};
use zip::ZipArchive;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Data {
    data: String,
    typ: String,
    device: String,
    pined: bool,
}

impl Data {
    pub fn new(data: String, typ: String, device: String, pined: bool) -> Self {
        Data {
            data,
            typ,
            device,
            pined,
        }
    }

    pub fn write_to_json(&self, tx: &Sender<(String, String)>) -> Result<(), io::Error> {
        let time = Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string();

        fs::create_dir_all(&get_path())?;

        let file_path = &get_path().join(&time);

        if self.typ.starts_with("image/") {
            self.save_image(&time)?;
        }

        let json_data = serde_json::to_string_pretty(self)?;

        let mut file = File::create(file_path)?;
        file.write_all(json_data.as_bytes())?;

        match tx.send((file_path.to_str().unwrap().into(), time)) {
            Ok(_) => (),
            Err(err) => warn!(
                "Failed to send file '{}' to channel: {}",
                file_path.display(),
                err
            ),
        }

        Ok(())
    }

    pub fn get_data(&self) -> Option<String> {
        if !self.typ.starts_with("image/") {
            Some(self.data.clone())
        } else {
            None
        }
    }

    pub fn get_image_thumbnail(&self, id: &DirEntry) -> Option<(Vec<u8>, (u32, u32))> {
        let mut path = get_path_image();
        let file_nema = format!("{}.png", id.file_name().to_str().unwrap());
        path.push(file_nema);

        let image = ImageReader::open(path).ok()?.decode().ok()?;

        let rgba = image.to_rgba8();

        let size = (rgba.width(), rgba.height());
        Some((rgba.into_raw(), size))
    }

    pub fn get_image(&self) -> Option<Vec<u8>> {
        if self.typ.starts_with("image/") {
            Some(general_purpose::STANDARD.decode(&self.data).unwrap())
        } else {
            None
        }
    }

    pub fn get_image_as_string(&self) -> Option<&str> {
        if self.typ.starts_with("image/") {
            Some(&self.data)
        } else {
            None
        }
    }

    pub fn get_pined(&self) -> bool {
        self.pined
    }

    pub fn save_image(&self, time: &str) -> Result<(), io::Error> {
        let mut path: PathBuf = crate::get_path();
        path.pop();
        let path = path.join("image");

        fs::create_dir_all(&path)?;

        let img_path = path.join(format!("{}.png", time));
        let mut img_file = File::create(img_path)?;

        let data: Vec<u8> = self
            .get_image()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Failed to get image data"))?;

        let image = image::load_from_memory(&data).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Image decode error: {e}"),
            )
        })?;

        let resized = image.thumbnail(128, 128);

        resized
            .write_to(&mut img_file, image::ImageFormat::Png)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Image write error: {e}")))?;

        Ok(())
    }

    pub fn get_meta_data(&self) -> Result<String, ()> {
        let mut display_text = String::new();

        if self.typ.starts_with("text") {
            if let Some(truncated_text) = self.get_data() {
                display_text = if truncated_text.len() > 30 {
                    format!("{}...", &truncated_text[..30])
                } else {
                    truncated_text
                }
            }
        } else {
            return Err(());
        }
        Ok(display_text)
    }
}

#[derive(Debug, Clone)]
pub struct UserData {
    data: Arc<Mutex<BTreeSet<String>>>,
}

impl UserData {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(BTreeSet::new())),
        }
    }

    pub fn build() -> Self {
        let mut temp = BTreeSet::new();

        fs::create_dir_all(&get_path()).unwrap();

        let folder_path = &get_path();

        if let Ok(entries) = fs::read_dir(folder_path.as_path()) {
            for entry in entries.flatten() {
                if let Some(file_name) = entry.file_name().to_str() {
                    temp.insert(file_name.to_string());
                }
            }
        }

        debug!("{:?}", temp);

        Self {
            data: Arc::new(Mutex::new(temp)),
        }
    }

    pub fn add(&self, id: String) {
        self.data.lock().unwrap().insert(id);
    }

    pub fn add_vec(&self, id: Vec<String>) {
        for id in id {
            self.data.lock().unwrap().insert(id);
        }
    }

    pub fn last_one(&self) -> String {
        self.data
            .lock()
            .unwrap()
            .last()
            .unwrap_or(&"".to_string())
            .clone()
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct UserCred {
    pub username: String,
    pub key: String,
}

impl UserCred {
    pub fn new(username: String, key: String) -> Self {
        Self { username, key }
    }
}

#[derive(Serialize, Deserialize)]
pub struct UserSettings {
    sync: Option<UserCred>,
    pub store_image: bool,
    pub click_on_quit: bool,
    encrept: Option<String>,
    pub intrevel: u32,
}

impl UserSettings {
    pub fn new() -> Self {
        Self {
            sync: None,
            store_image: true,
            encrept: None,
            click_on_quit: true,
            intrevel: 3,
        }
    }

    pub fn get_sync(&self) -> &Option<UserCred> {
        &self.sync
    }
}

#[derive(Debug, Clone)]
pub struct Pending {
    data: Arc<Mutex<Vec<(String, String)>>>,
}

impl Pending {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn add(&self, id: (String, String)) {
        self.data.lock().unwrap().push(id);
    }

    pub fn is_empty(&self) -> bool {
        self.data.lock().unwrap().is_empty()
    }

    pub fn get(&self) -> Option<(&str, &str)> {
        None
    }

    pub fn remove(&self) {}
}

pub fn get_path() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        return [home.as_str(), ".local/share/clippy/data"].iter().collect();
    }

    #[cfg(target_os = "macos")]
    {
        let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        return [home.as_str(), ".local/share/clippy/data"].iter().collect();
    }

    #[cfg(target_os = "windows")]
    {
        let home = env::var("APPDATA").unwrap_or_else(|_| "C:\\Users\\Public\\AppData".to_string());
        return [home.as_str(), "clippy\\data"].iter().collect();
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        compile_error!("Unsupported operating system");
    }
}

pub fn get_path_image() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        return [home.as_str(), ".local/share/clippy/image"]
            .iter()
            .collect();
    }

    #[cfg(target_os = "macos")]
    {
        let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        return [home.as_str(), ".local/share/clippy/image"]
            .iter()
            .collect();
    }

    #[cfg(target_os = "windows")]
    {
        let home = env::var("APPDATA").unwrap_or_else(|_| "C:\\Users\\Public\\AppData".to_string());
        return [home.as_str(), "clippy\\image"].iter().collect();
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        compile_error!("Unsupported operating system");
    }
}

pub fn extract_zip(data: Bytes) -> Result<Vec<String>, zip::result::ZipError> {
    let target_dir = get_path();
    let mut id = Vec::new();
    let cursor = Cursor::new(data);
    let mut archive = ZipArchive::new(cursor)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let file_name = file.name();
        id.push(file_name.to_string());
        let mut out_path = target_dir.clone();
        out_path.push(file_name);

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut outfile = File::create(&out_path)?;
        std::io::copy(&mut file, &mut outfile)?;
    }

    match store_image(&id, target_dir) {
        Ok(_) => (),
        Err(err) => error!("Failed to store image file: {}", err),
    }
    Ok(id)
}

pub fn store_image(id: &[String], target_dir: PathBuf) -> Result<(), Error> {
    for i in id {
        let mut path = target_dir.clone();
        path.push(i);

        let file = fs::read_to_string(path)?;
        let data: Data = serde_json::from_str(&file)?;

        if data.typ.starts_with("image/") {
            data.save_image(i)?;
        }
    }
    Ok(())
}

pub fn set_global_bool(value: bool) {
    let mut path = get_path();
    if let Err(e) = fs::create_dir_all(path.parent().unwrap()) {
        error!("Failed to create directories: {}", e);
        return;
    }

    path.pop();
    let path = Path::new(&path).join("OK");

    if value {
        if let Err(e) = fs::remove_file(&path) {
            error!("Failed to delete state file: {}", e);
        }
    } else {
        if let Err(e) = fs::File::create(&path) {
            error!("Failed to create state file: {}", e);
        }
    }
}

pub fn get_global_bool() -> bool {
    let mut path = get_path();
    path.pop();
    let path = Path::new(&path).join("OK");
    !path.exists()
}
