use clipboard_rs::{ClipboardWatcher, ClipboardWatcherContext};
use clippy::read_clipboard::{self, read_wayland_clipboard};
use std::{env, process};
use wayland_clipboard_listener::{WlClipboardPasteStream, WlListenType};

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
    let mut stream = WlClipboardPasteStream::init(WlListenType::ListenOnCopy).unwrap();

    for _ in stream.paste_stream().flatten().flatten() {
        match read_wayland_clipboard() {
            Ok(_) => (),
            Err(err) => println!("{}", err),
        }
    }
}

fn read() {
    let manager = read_clipboard::Manager::new();

    let mut watcher = ClipboardWatcherContext::new().unwrap();

    let _watcher_shutdown = watcher.add_handler(manager).get_shutdown_channel();

    if env::var("DEBUG").is_ok() {
        println!("start watch!");
    }
    watcher.start_watch();
}
