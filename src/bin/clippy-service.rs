use clipboard_rs::{ClipboardWatcher, ClipboardWatcherContext};
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
                    read_clipboard::write_to_json(old_data.to_owned(), typ, "os".to_owned());
                }
            }
            Err(_) => (),
        }

        thread::sleep(Duration::from_millis(1000));
    }
}

fn read() {
    let manager = read_clipboard::Manager::new();

    let mut watcher = ClipboardWatcherContext::new().unwrap();

    let _watcher_shutdown = watcher.add_handler(manager).get_shutdown_channel();

    println!("start watch!");
    watcher.start_watch();
}
