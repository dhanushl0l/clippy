use clipboard_rs::{ClipboardWatcher, ClipboardWatcherContext};
use clippy::Pending;
use clippy::UserData;
use clippy::http;
use clippy::http::health;
use clippy::http::send;
use clippy::read_clipboard;
use core::time;
use env_logger::{Builder, Env};
use reqwest::blocking::Client;
use std::error::Error;
use std::sync::Arc;
use std::sync::mpsc::{self, Sender};
use std::{env, process, thread};

#[cfg(target_os = "linux")]
use clippy::read_clipboard::read_wayland_clipboard;

#[cfg(target_os = "linux")]
use wayland_clipboard_listener::{WlClipboardPasteStream, WlListenType};

fn main() {
    Builder::from_env(Env::default().filter_or("LOG", "info")).init();

    let (tx, rx) = mpsc::channel::<(String, String)>();
    let user_data = UserData::build();
    let user_data1 = user_data.clone();
    let pending = Pending::new();
    let pending1 = pending.clone();
    let client = Arc::new(Client::new());
    let client1 = client.clone();

    thread::spawn(move || {
        loop {
            while let Some((path, id)) = pending.get() {
                match send(&path, &id, &user_data, &client) {
                    Ok(_) => pending.remove(),
                    Err(err) => {
                        eprintln!("{:?}", err);
                        health(&client);
                        continue;
                    }
                };
            }

            match http::state(&user_data, &client) {
                Ok(result) => {
                    if !result {
                        match http::download(&user_data, &client) {
                            Ok(_) => (),
                            Err(err) => eprintln!("{}", err),
                        };
                    } else {
                        println!("every thihng is uptodate");
                        thread::sleep(time::Duration::from_secs(3));
                    }
                }
                Err(err) => {
                    eprintln!("11111{:?}", err);
                    health(&client);
                }
            }
        }
    });

    thread::spawn(move || {
        for (path, id) in rx {
            match send(&path, &id, &user_data1, &client1) {
                Ok(_) => (),
                Err(err) => {
                    pending1.add((id, path));
                    eprintln!("{}", err)
                }
            };
        }
    });
    run(&tx)
}

#[cfg(target_os = "linux")]
fn run(tx: &Sender<(String, String)>) {
    if env::var("WAYLAND_DISPLAY").is_ok() {
        read_clipboard_wayland(tx)
    } else if env::var("DISPLAY").is_ok() {
        match read_clipboard(tx) {
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

// need to find a way to monitor clipboard changes in wayland the current way is not optimal
#[cfg(target_os = "linux")]
fn read_clipboard_wayland(tx: &Sender<(String, String)>) {
    let mut stream = WlClipboardPasteStream::init(WlListenType::ListenOnCopy).unwrap();

    for _ in stream.paste_stream().flatten().flatten() {
        match read_wayland_clipboard(tx) {
            Ok(_) => (),
            Err(err) => println!("{}", err),
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn run(tx: &Sender<(String, String)>) {
    match read_clipboard(tx) {
        Ok(_) => (),
        Err(err) => {
            eprintln!("Failed to initialize clipboard listener\n{}", err);
            process::exit(1);
        }
    };
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn run(tx: Sender<(String, String)>) {
    eprintln!("Unsupported OS");
    process::exit(1);
}

fn read_clipboard(tx: &Sender<(String, String)>) -> Result<(), Box<dyn Error + Send + Sync>> {
    let manager = read_clipboard::Manager::new(tx);

    let mut watcher = ClipboardWatcherContext::new()?;

    let _watcher_shutdown = watcher.add_handler(manager).get_shutdown_channel();

    if env::var("DEBUG").is_ok() {
        println!("start watch!");
    }
    watcher.start_watch();
    Ok(())
}
