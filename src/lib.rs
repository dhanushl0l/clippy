pub mod http;
pub mod read_clipboard;
pub mod write_clipboard;

use bytes::Bytes;
use chrono::prelude::Utc;
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Write};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::{
    collections::BTreeSet,
    env,
    fs::File,
    fs::{self},
    io::{self},
    path::PathBuf,
    process,
};
use zip::ZipArchive;

#[derive(Serialize, Deserialize, Debug)]
pub struct Data {
    data: Vec<u8>,
    typ: String,
    device: String,
    pined: bool,
}

pub static PATH: &str = env::consts::OS;

impl Data {
    pub fn new(data: Vec<u8>, typ: String, device: String, pined: bool) -> Self {
        Data {
            data,
            typ,
            device,
            pined,
        }
    }

    pub fn write_to_json(&self, tx: &Sender<(String, String)>) -> Result<(), io::Error> {
        let time = Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string();

        fs::create_dir_all(&get_path(PATH))?;

        let file_path = &get_path(PATH).join(&time);

        if self.typ.starts_with("image/") {
            self.save_image(&time)?;
        }

        let json_data = serde_json::to_string_pretty(self)?;

        let mut file = File::create(file_path)?;
        file.write_all(json_data.as_bytes())?;

        match tx.send((file_path.to_str().unwrap().into(), time)) {
            Ok(_) => (),
            Err(err) => eprintln!("{}", err),
        }

        Ok(())
    }

    pub fn get_data(&self) -> Option<String> {
        if self.typ.starts_with("image/") {
            Some(String::from_utf8_lossy(&self.data).into_owned())
        } else {
            None
        }
    }

    pub fn get_pined(&self) -> bool {
        self.pined
    }

    pub fn save_image(&self, time: &str) -> Result<(), io::Error> {
        let mut path: PathBuf = crate::get_path(PATH);
        path.pop();
        let path: PathBuf = path.join("image").join(time);

        fs::create_dir_all(&path)?;

        let mut img_file = File::create(path.join("img.png"))?;

        img_file.write_all(&self.data)?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct UserData {
    data: Arc<Mutex<BTreeSet<String>>>,
}

impl UserData {
    pub fn new() -> Self {
        let mut temp = BTreeSet::new();

        fs::create_dir_all(&get_path(PATH)).unwrap();

        let folder_path = &get_path(PATH);

        if let Ok(entries) = fs::read_dir(folder_path.as_path()) {
            for entry in entries.flatten() {
                if let Some(file_name) = entry.file_name().to_str() {
                    temp.insert(file_name.to_string());
                }
            }
        }

        println!("{:?}", temp);

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

#[derive(Serialize)]
pub struct UserCred {
    pub username: String,
    pub key: String,
    pub id: String,
}

impl UserCred {
    pub fn new(username: String, key: String, id: String) -> Self {
        Self { username, key, id }
    }
}

pub fn get_path(os: &str) -> PathBuf {
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

pub fn extract_zip(data: Bytes) -> Result<Vec<String>, zip::result::ZipError> {
    let target_dir = get_path(PATH);
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

    Ok(id)
}
