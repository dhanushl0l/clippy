use std::{
    fs::File,
    io::{self, Read},
    path::PathBuf,
};

use clippy::Data;
use serde::{Deserialize, Serialize};

pub mod clipboard_wg;
pub mod state;
pub mod update;
pub mod view;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIData {
    data: String,
    pub typ: String,
    device: String,
    path: PathBuf,
    pub pined: bool,
}

impl UIData {
    pub fn build(path: &PathBuf) -> Result<Self, io::Error> {
        let mut buf = String::new();
        File::open(path)?.read_to_string(&mut buf)?;

        let data: Data = serde_json::from_str(&buf)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let data: UIData = Self {
            data: data.get_meta_data(),
            typ: data.typ,
            device: data.device,
            path: path.clone(),
            pined: data.pined,
        };

        Ok(data)
    }
}
