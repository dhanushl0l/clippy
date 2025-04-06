use std::{error::Error, fs::File, io::Read};

use crate::{Data, UserData, get_path};

use clipboard_rs::{Clipboard, ClipboardContext, RustImageData, common::RustImage};
#[cfg(target_os = "linux")]
use wl_clipboard_rs::copy::{ClipboardType, MimeType, Options, Source};

#[cfg(target_os = "linux")]
pub fn copy_to_clipboard(userdata: &UserData) -> Result<(), String> {
    use crate::set_global_bool;

    let mut opts = Options::new();
    opts.clipboard(ClipboardType::Regular);

    let data = read_data(userdata.last_one());
    set_global_bool(false);

    let typ = data.typ;
    let data = data.data.into_boxed_slice();

    if typ.starts_with("image/") {
        let a = opts.clone().copy(
            Source::Bytes(data.clone()),
            MimeType::Specific("image/png".to_string()),
        );
    } else {
        let a = opts
            .clone()
            .copy(Source::Bytes(data.clone()), MimeType::Text);
    }
    Ok(())
}

pub fn copy_to_clipboard(userdata: &UserData) -> Result<(), Box<dyn Error + Send + Sync>> {
    use crate::set_global_bool;
    let data = read_data(userdata.last_one());
    let ctx = ClipboardContext::new()?;

    set_global_bool(false);

    let typ = data.typ;
    let data = data.data;

    if typ.starts_with("image/") {
        ctx.set_image(RustImageData::from_bytes(&data)?);
    } else {
        ctx.set_text(String::from_utf8(data)?);
    }

    Ok(())
}

fn read_data(file: String) -> Data {
    let target = get_path().join(file);
    let mut file = File::open(target).unwrap();

    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap_or(0);

    let data: Data = serde_json::from_str(&contents).unwrap_or(Data::new(
        vec![1],
        "empty".to_string(),
        "os".to_string(),
        true,
    ));

    data
}
