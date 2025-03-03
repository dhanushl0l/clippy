use clippy::read_clipboard;
use std::{env, process, thread, time::Duration};

fn main() {
    match env::consts::OS {
        "linux" => {
            if env::var("WAYLAND_DISPLAY").is_ok() {
                read_wayland()
            } else if env::var("DISPLAY").is_ok() {
                read();
            } else {
                eprint!("No display server detected");
                process::exit(1);
            }
        }
        "windows" => read(),
        "macos" => read(),
        _ => {
            eprintln!("unsuported hardware");
            process::exit(1);
        }
    }
}

// need to find a way to monitor clipboard changes in wayland the current way is not optimized
fn read_wayland() {
    let mut old_data = Vec::new();
    loop {
        let a = read_clipboard::read_wayland_clipboard();
        match a {
            Ok((data, typ)) => {
                if data != old_data {
                    old_data = data;
                    if env::var("DEBUG").is_ok() {
                        println!(
                            "{:?}",
                            String::from_utf8(old_data.to_owned()).expect("Invalid UTF-8")
                        );
                    }
                    write_to_json(old_data.to_owned(), typ, "os".to_owned());
                }
            }
            Err(_) => (),
        }

        thread::sleep(Duration::from_millis(1000));
    }
}

use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext,
};

struct Manager {
    ctx: ClipboardContext,
}

impl Manager {
    pub fn new() -> Self {
        let ctx = ClipboardContext::new().unwrap();
        Manager { ctx }
    }
}

impl ClipboardHandler for Manager {
    fn on_clipboard_change(&mut self) {
        let ctx = &self.ctx;
        let types = ctx.available_formats().unwrap();
        if env::var("DEBUG").is_ok() {
            println!("{:?}", types);

            let content = ctx.get_text().unwrap_or("".to_string());

            println!("txt={}", content);
        }

        if let Ok(val) = ctx.get_image() {
            unimplemented!("");
        } else if let Ok(val) = ctx.get_text() {
            write_to_json(val.into_bytes(), String::from("String"), String::from("os"));
        }
    }
}

fn read() {
    let manager = Manager::new();

    let mut watcher = ClipboardWatcherContext::new().unwrap();

    let _watcher_shutdown = watcher.add_handler(manager).get_shutdown_channel();

    println!("start watch!");
    watcher.start_watch();
}

fn write_to_json(data: Vec<u8>, typ: String, device: String) {
    let data = read_clipboard::Data::new(data, typ, device);
    match data.write_to_json() {
        Ok(_) => (),
        Err(err) => eprintln!("{}", err),
    }
}
