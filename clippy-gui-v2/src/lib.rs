use iced::widget::image::Handle;
use std::{
    fs::File,
    io::{self, Read},
    path::PathBuf,
};

use clippy::Data;
use serde::{Deserialize, Serialize};

pub mod clipboard_wg;
pub mod footer;
pub mod header;
pub mod state;
pub mod update;
pub mod view;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIData {
    data: MetaData,
    pub typ: String,
    device: String,
    path: PathBuf,
    pub pined: bool,
    pub sync: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetaData {
    Text(String),
    Image(u32, u32, Vec<u8>),
}

impl UIData {
    pub fn build(path: &PathBuf) -> Result<Self, io::Error> {
        let mut buf = String::new();
        File::open(path)?.read_to_string(&mut buf)?;

        let data: Data = serde_json::from_str(&buf)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let metadata = if data.typ.ends_with("png") {
            let (image, size) = data.get_image_thumbnail(path).unwrap();
            MetaData::Image(size.0, size.1, image)
        } else {
            MetaData::Text(data.get_meta_data())
        };
        let data: UIData = Self {
            data: metadata,
            typ: data.typ,
            device: data.device,
            path: path.clone(),
            pined: data.pined,
            sync: true,
        };

        Ok(data)
    }
}
