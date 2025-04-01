use std::{fs::File, io::Read};

use crate::{Data, PATH, UserData, get_path};
use wl_clipboard_rs::copy::{ClipboardType, MimeType, Options, Source};

#[cfg(target_os = "linux")]
pub fn copy_to_clipboard(userdata: &UserData) -> Result<(), String> {
    let mut opts = Options::new();
    opts.clipboard(ClipboardType::Regular);

    let data = read_data(userdata.last_one());

    if opts
        .clone()
        .copy(
            Source::Bytes(data.clone()),
            MimeType::Specific("text/clippy".to_string()),
        )
        .is_err()
    {
        opts.copy(Source::Bytes(data), MimeType::Autodetect)
            .map_err(|e| format!("Failed to copy: {}", e))?;
    }

    Ok(())
}

fn read_data(file: String) -> Box<[u8]> {
    let target = get_path(PATH).join(file);
    let mut file = File::open(target).unwrap();

    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();

    let data: Data = serde_json::from_str(&contents).unwrap();

    data.data.into_boxed_slice()
}
