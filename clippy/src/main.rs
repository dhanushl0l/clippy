use clipboard_rs::{ClipboardWatcher, ClipboardWatcherContext};
use clippy::user::start_cloud;
use clippy::{UserSettings, read_clipboard};
use env_logger::{Builder, Env};
use log::debug;
use log::{error, info, warn};
use std::error::Error;
use std::{env, process};
use tokio::sync::mpsc::Sender;

#[cfg(target_os = "linux")]
use clippy::read_clipboard::read_wayland_clipboard;

#[cfg(target_os = "linux")]
use wayland_clipboard_listener::{WlClipboardPasteStream, WlListenType};

#[cfg(target_os = "linux")]
fn run(tx: &Sender<(String, String)>) {
    if env::var("WAYLAND_DISPLAY").is_ok() {
        read_clipboard_wayland(tx)
    } else if env::var("DISPLAY").is_ok() {
        match read_clipboard(tx) {
            Ok(_) => (),
            Err(err) => {
                error!("Failed to initialize clipboard listener\n{}", err);
                process::exit(1);
            }
        };
    } else {
        error!("No display server detected");
        process::exit(1);
    }
}

// need to find a way to monitor clipboard changes in wayland the current way is not optimal
#[cfg(target_os = "linux")]
fn read_clipboard_wayland(tx: &Sender<(String, String)>) {
    let mut stream = WlClipboardPasteStream::init(WlListenType::ListenOnCopy).unwrap();

    for _ in stream.paste_stream().flatten().flatten() {
        match read_wayland_clipboard(tx) {
            Ok(_) => (),
            Err(err) => warn!("{}", err),
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn run(tx: &Sender<(String, String)>) {
    match read_clipboard(tx) {
        Ok(_) => (),
        Err(err) => {
            error!("Failed to initialize clipboard listener\n{}", err);
            process::exit(1);
        }
    };
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn run(tx: Sender<(String, String)>) {
    error!("Unsupported OS");
    process::exit(1);
}

fn read_clipboard(tx: &Sender<(String, String)>) -> Result<(), Box<dyn Error + Send + Sync>> {
    let manager = read_clipboard::Manager::new(tx);

    let mut watcher = ClipboardWatcherContext::new()?;

    let _watcher_shutdown = watcher.add_handler(manager).get_shutdown_channel();

    debug!("start watch clipboard!");
    watcher.start_watch();
    Ok(())
}

fn main() {
    Builder::from_env(Env::default().filter_or("LOG", "info")).init();

    let (tx, rx) = tokio::sync::mpsc::channel::<(String, String)>(30);

    match UserSettings::build_user() {
        Ok(usersettings) => {
            if let Some(sync) = usersettings.get_sync() {
                start_cloud(rx, sync.clone());
            }
        }
        Err(err) => {
            info!("user not logged in: {}", err);
        }
    }

    run(&tx)
}
