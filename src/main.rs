use std::{env, process, thread, time::Duration};

use read_clipboard::{self, Data};
fn main() {
    match env::consts::OS {
        "linux" => {
            if env::var("WAYLAND_DISPLAY").is_ok() {
                read_wayland()
            } else if env::var("DISPLAY").is_ok() {
                unimplemented!("hell")
            } else {
                eprint!("No display server detected");
                process::exit(1);
            }
        }
        "windows" => unimplemented!(""),
        "macos" => unimplemented!(""),
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

fn write_to_json(data: Vec<u8>, typ: String, device: String) {
    let data = Data::new(data, typ, device);
    match data.write_to_json() {
        Ok(_) => (),
        Err(err) => eprintln!("{}", err),
    }
}
