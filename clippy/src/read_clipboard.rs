use crate::{Data, get_global_bool, set_global_bool};
use clipboard_rs::common::RustImage;
use clipboard_rs::{Clipboard, ClipboardContext, ClipboardHandler};
use std::sync::mpsc::Sender;

#[cfg(target_os = "linux")]
pub fn read_wayland_clipboar1d(tx: &Sender<(String, String)>) -> Result<(), Error> {
    use crate::{get_global_bool, set_global_bool};
    use std::collections::HashSet;
    use std::io::Read;
    use wl_clipboard_rs::paste::{
        ClipboardType, Error, MimeType, Seat, get_contents, get_mime_types,
    };

    if get_global_bool() {
        let typ: HashSet<String> = get_mime_types(ClipboardType::Regular, Seat::Unspecified)?;

        for i in &typ {
            if i == "text/clippy" {
                return Ok(());
            }
        }

        let preferred_formats = [
            "image/png",
            "image/jpeg",
            "image/jxl",
            "image/tiff",
            "image/bmp",
            "text/plain;charset=utf-8",
            "text/plain",
            "STRING",
            "UTF8_STRING",
            "text/uri-list",
        ];

        let mut main_type = String::new();
        // Check for preferred formats in order of priority
        for &format in &preferred_formats {
            if let Some(fallback) = typ.iter().find(|m| *m == format) {
                main_type = fallback.to_owned();
                break;
            }
        }

        let mime_type = MimeType::Specific(&main_type);
        let (mut dat, typ) = get_contents(ClipboardType::Regular, Seat::Unspecified, mime_type)?;
        let mut vec = Vec::new();
        let _ = dat.read_to_end(&mut vec);

        parse_wayland_clipboard(typ, vec, tx);
    } else {
        set_global_bool(true);
    }
    Ok(())
}

pub struct Manager<'a> {
    ctx: ClipboardContext,
    tx: &'a Sender<(String, String)>,
}

impl<'a> Manager<'a> {
    pub fn new(tx: &'a Sender<(String, String)>) -> Self {
        let ctx = ClipboardContext::new().unwrap();
        Manager { ctx, tx }
    }
}

impl<'a> ClipboardHandler for Manager<'a> {
    fn on_clipboard_change(&mut self) {
        if get_global_bool() {
            let ctx = &self.ctx;
            let types = ctx.available_formats().unwrap();

            eprintln!("{:?}", types);

            if let Ok(val) = ctx.get_image() {
                println!("img");
                write_to_json(
                    val.to_png().unwrap().get_bytes().to_vec(),
                    String::from("image/png"),
                    String::from("os"),
                    &self.tx,
                );
            } else if let Ok(val) = ctx.get_text() {
                write_to_json(
                    val.into_bytes(),
                    String::from("String"),
                    String::from("os"),
                    &self.tx,
                );
            }
        } else {
            set_global_bool(true);
        }
    }
}

pub fn write_to_json(data: Vec<u8>, typ: String, device: String, tx: &Sender<(String, String)>) {
    let data = Data::new(data, typ, device, false);
    match data.write_to_json(tx) {
        Ok(_) => (),
        Err(err) => eprintln!("{}", err),
    }
}

pub fn parse_wayland_clipboard(typ: String, data: Vec<u8>, tx: &Sender<(String, String)>) {
    println!("{}", typ);

    let result = Data::new(data, typ, "os".to_owned(), false);
    match result.write_to_json(tx) {
        Ok(_) => (),
        Err(err) => eprintln!("{:?}", err),
    }
}
