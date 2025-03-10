pub mod read_clipboard;

use chrono::prelude::Utc;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::{
    env,
    fs::File,
    fs::{self},
    io::{self},
    path::PathBuf,
    process,
};

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

    pub fn write_to_json(&self) -> Result<(), io::Error> {
        let time = Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string();

        fs::create_dir_all(&get_path(PATH))?;

        let file_path = &get_path(PATH).join(format!("{}.json", time));

        let json_data = serde_json::to_string_pretty(self)?;

        let mut file = File::create(file_path)?;
        file.write_all(json_data.as_bytes())?;

        Ok(())
    }

    pub fn get_data(&self) -> String {
        String::from_utf8_lossy(&self.data).into_owned()
    }

    pub fn get_pined(&self) -> bool {
        self.pined
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
