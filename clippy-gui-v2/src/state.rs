use std::fs;

use clippy::{Data, UserSettings, get_path, get_path_local, get_path_pending};

use crate::UIData;

#[derive(Debug, Clone)]
pub struct ClipboardState {
    pub clipboard_data: Vec<UIData>,
    pub settings: UserSettings,
    pub value: f32,
}

impl ClipboardState {
    pub fn build() -> Self {
        let mut clipboard_data = vec![];
        let path = get_path_pending();
        let mut count = 0;
        for i in fs::read_dir(path).unwrap() {
            count += 1;
            if let Ok(entry) = i {
                let path = entry.path();
                if path.is_file() {
                    let data = UIData::build(&path);
                    match data {
                        Ok(val) => {
                            clipboard_data.push(val);
                        }
                        Err(e) => {
                            println!("err: {:?}", e)
                        }
                    }
                }
            } else {
                println!("error")
            }
            if count > 20 {
                break;
            }
        }
        let data = Self {
            clipboard_data,
            settings: UserSettings::new(),
            value: 1.1,
        };
        data
    }
}
impl Default for ClipboardState {
    fn default() -> Self {
        Self {
            clipboard_data: vec![],
            settings: UserSettings::new(),
            value: 1.1,
        }
    }
}
