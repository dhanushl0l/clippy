#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use clipboard_rs::{ClipboardWatcher, ClipboardWatcherContext};
use clippy::user::start_cloud;
use clippy::{Pending, UserSettings, get_path_local, read_clipboard, watch_for_next_clip_write};
use env_logger::{Builder, Env};
use fs4::fs_std::FileExt;
use log::debug;
use log::{error, info, warn};
use std::error::Error;
use std::fs::File;
use std::sync::Arc;
use std::time::Duration;
use std::{process, thread};

#[cfg(target_os = "linux")]
use clippy::read_clipboard::read_wayland_clipboard;

#[cfg(target_os = "linux")]
use wayland_clipboard_listener::{WlClipboardPasteStream, WlListenType};

#[cfg(target_os = "linux")]
fn run(pending: Arc<Pending>) {
    use std::env;

    if env::var("WAYLAND_DISPLAY").is_ok() {
        read_clipboard_wayland(pending)
    } else if env::var("DISPLAY").is_ok() {
        match read_clipboard(pending) {
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
fn read_clipboard_wayland(pending: Arc<Pending>) {
    let mut stream = WlClipboardPasteStream::init(WlListenType::ListenOnCopy).unwrap();

    for _ in stream.paste_stream().flatten().flatten() {
        match read_wayland_clipboard(pending.clone()) {
            Ok(_) => (),
            Err(err) => warn!("{}", err),
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn run(pending: Arc<Pending>) {
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

fn read_clipboard(pending: Arc<Pending>) -> Result<(), Box<dyn Error + Send + Sync>> {
    let manager = read_clipboard::Manager::new(pending);

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

    let pending = Arc::new(Pending::build().unwrap());

    let mut paste_on_click = false;
    match UserSettings::build_user() {
        Ok(usersettings) => {
            paste_on_click = usersettings.paste_on_click;
            if !usersettings.disable_sync {
                if let Some(sync) = usersettings.get_sync() {
                    start_cloud(pending.clone(), sync.clone(), usersettings);
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
        thread::sleep(Duration::from_secs(1));
        let path = get_path_local();
        thread::spawn(move || {
            watch_for_next_clip_write(path, paste_on_click);
        });
    }

    run(pending.clone())
}
