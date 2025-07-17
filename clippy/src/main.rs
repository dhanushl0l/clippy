#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use clipboard_rs::{ClipboardWatcher, ClipboardWatcherContext};
use clippy::ipc::ipc::{ipc_check, startup};
use clippy::user::start_cloud;
use clippy::{MessageChannel, UserSettings, read_clipboard};
use env_logger::{Builder, Env};
use log::debug;
use log::{error, info};
use std::error::Error;
use std::{process, thread};
use tokio::sync::mpsc::Sender;

#[cfg(target_os = "linux")]
use clippy::read_clipboard::read_wayland_clipboard;
#[cfg(target_os = "linux")]
use wayland_clipboard_listener::{WlClipboardPasteStream, WlListenType};

#[cfg(target_os = "linux")]
fn run(tx: &Sender<MessageChannel>) {
    use std::env;

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
fn read_clipboard_wayland(tx: &Sender<MessageChannel>) {
    let mut stream = WlClipboardPasteStream::init(WlListenType::ListenOnCopy).unwrap();

    for _ in stream.paste_stream().flatten().flatten() {
        match read_wayland_clipboard(tx) {
            Ok(_) => (),
            Err(err) => warn!("{}", err),
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn run(tx: &Sender<MessageChannel>) {
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

fn read_clipboard(tx: &Sender<MessageChannel>) -> Result<(), Box<dyn Error + Send + Sync>> {
    let manager = read_clipboard::Manager::new(tx);

    let mut watcher = ClipboardWatcherContext::new()?;

    let _watcher_shutdown = watcher.add_handler(manager).get_shutdown_channel();

    debug!("start watch clipboard!");
    watcher.start_watch();
    Ok(())
}

fn main() {
    Builder::from_env(Env::default().filter_or("LOG", "info")).init();
    let channel = match startup() {
        Ok(x) => {
            debug!("Process startup success");
            x
        }
        Err(e) => {
            error!("Another instence of the app is active stop it and try again");
            error!("{}", e);
            process::exit(1);
        }
    };

    let (tx, rx) = tokio::sync::mpsc::channel::<MessageChannel>(30);

    match UserSettings::build_user() {
        Ok(usersettings) => {
            if !usersettings.disable_sync {
                if let Some(sync) = usersettings.get_sync() {
                    start_cloud(rx, sync.clone(), usersettings);
                } else {
                }
            } else {
            }
        }
        Err(err) => {
            info!("user not logged in: {}", err);
        }
    }

    // this thread reads the gui clipboard entry && settings change
    {
        let tx_c = tx.clone();
        thread::spawn(move || {
            ipc_check(channel, &tx_c);
        });
    }

    run(&tx)
}
