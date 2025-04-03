use std::{fs::File, io::Read};

use crate::{Data, PATH, UserData, get_path};
use wl_clipboard_rs::copy::{ClipboardType, MimeType, Options, Source};

#[cfg(target_os = "linux")]
pub fn copy_to_clipboard(userdata: &UserData) -> Result<(), String> {
    use crate::set_global_bool;

    let mut opts = Options::new();
    opts.clipboard(ClipboardType::Regular);

    let data = read_data(userdata.last_one());

    let typ = data.typ;
    let data = data.data.into_boxed_slice();

    set_global_bool(false);
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

fn read_data(file: String) -> Data {
    let target = get_path(PATH).join(file);
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
