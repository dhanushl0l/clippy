#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use clipboard_rs::{ClipboardWatcher, ClipboardWatcherContext};
use clippy::user::start_cloud;
use clippy::{UserSettings, get_path_local, read_clipboard, watch_for_next_clip_write};
use env_logger::{Builder, Env};
use fs4::fs_std::FileExt;
use log::debug;
use log::{error, info, warn};
use std::error::Error;
use std::fs::File;
use std::{process, thread};
use tokio::sync::mpsc::Sender;

#[cfg(target_os = "linux")]
use clippy::read_clipboard::read_wayland_clipboard;

#[cfg(target_os = "linux")]
use wayland_clipboard_listener::{WlClipboardPasteStream, WlListenType};

#[cfg(target_os = "linux")]
fn run(tx: &Sender<(String, String, String)>) {
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
fn read_clipboard_wayland(tx: &Sender<(String, String, String)>) {
    let mut stream = WlClipboardPasteStream::init(WlListenType::ListenOnCopy).unwrap();

    for _ in stream.paste_stream().flatten().flatten() {
        match read_wayland_clipboard(tx) {
            Ok(_) => (),
            Err(err) => warn!("{}", err),
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn run(tx: &Sender<(String, String, String)>) {
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

fn read_clipboard(
    tx: &Sender<(String, String, String)>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let manager = read_clipboard::Manager::new(tx);

    let mut watcher = ClipboardWatcherContext::new()?;

    let _watcher_shutdown = watcher.add_handler(manager).get_shutdown_channel();

    debug!("start watch clipboard!");
    watcher.start_watch();
    Ok(())
}

fn setup(file: &File) -> Result<(), Box<dyn Error>> {
    Builder::from_env(Env::default().filter_or("LOG", "info")).init();

    if std::env::var("IGNORE_STARTUP_LOCK").is_ok() {
        warn!("Startup lock ignored due to IGNORE_STARTUP_LOCK=1");
        return Ok(());
    }

    match file.try_lock_exclusive()? {
        true => {
            debug!("Lock acquired!");
        }
        false => {
            error!(
                "Another instance of the app is already running. If you're facing issues and the process is not actually running, set the environment variable IGNORE_STARTUP_LOCK=1 to override the lock."
            );
            process::exit(1);
        }
    }
    Ok(())
}

fn main() {
    let mut path = get_path_local();
    path.push("CLIPPY.LOCK");

    if !path.exists() {
        File::create(&path).unwrap();
    }
    let file = File::open(path).unwrap();
    match setup(&file) {
        Ok(_) => {
            debug!("Process startup success");
        }
        Err(err) => {
            error!("Unable to start the app: {}", err);
            process::exit(1);
        }
    }

    let (tx, rx) = tokio::sync::mpsc::channel::<(String, String, String)>(30);

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
        let path = get_path_local();
        thread::spawn(|| {
            watch_for_next_clip_write(path);
        });
    }

    run(&tx)
}
