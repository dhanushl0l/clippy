use clipboard_rs::{ClipboardWatcher, ClipboardWatcherContext};
use clippy::read_clipboard;
use std::error::Error;
use std::{env, process};

#[cfg(target_os = "linux")]
use clippy::read_clipboard::read_wayland_clipboard;

#[cfg(target_os = "linux")]
use wayland_clipboard_listener::{WlClipboardPasteStream, WlListenType};

fn main() {
    run()
}

#[cfg(target_os = "linux")]
fn run() {
    if env::var("WAYLAND_DISPLAY").is_ok() {
        read_clipboard_wayland()
    } else if env::var("DISPLAY").is_ok() {
        match read_clipboard() {
            Ok(_) => (),
            Err(err) => {
                eprintln!("Failed to initialize clipboard listener\n{}", err);
                process::exit(1);
            }
        };
    } else {
        eprint!("No display server detected");
        process::exit(1);
    }
}

// need to find a way to monitor clipboard changes in wayland the current way is not optimized
#[cfg(target_os = "linux")]
fn read_clipboard_wayland() {
    let mut stream = WlClipboardPasteStream::init(WlListenType::ListenOnCopy).unwrap();

    for _ in stream.paste_stream().flatten().flatten() {
        match read_wayland_clipboard() {
            Ok(_) => (),
            Err(err) => println!("{}", err),
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn run() {
    match read_clipboard() {
        Ok(_) => (),
        Err(err) => {
            eprintln!("Failed to initialize clipboard listener\n{}", err);
            process::exit(1);
        }
    };
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn run() {
    eprintln!("Unsupported OS");
    process::exit(1);
}

fn read_clipboard() -> Result<(), Box<dyn Error + Send + Sync>> {
    let manager = read_clipboard::Manager::new();

    let mut watcher = ClipboardWatcherContext::new()?;

    let _watcher_shutdown = watcher.add_handler(manager).get_shutdown_channel();

    if env::var("DEBUG").is_ok() {
        println!("start watch!");
    }
    watcher.start_watch();
    Ok(())
}
