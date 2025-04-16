use crate::{
    Pending, UserData, UserSettings,
    encryption_decryption::{decrypt_file, encrept_file},
    get_path, http,
};
use log::{info, warn};
use reqwest::blocking::Client;
use std::{
    error::Error,
    fs::{self, create_dir},
    sync::{Arc, mpsc::Receiver},
    thread, time,
};

pub fn start_cloud(rx: Receiver<(String, String)>) {
    let user_data = UserData::build();
    let user_data1 = user_data.clone();
    let pending = Pending::new();
    let pending1 = pending.clone();
    let client = Arc::new(Client::new());
    let client1 = client.clone();

    thread::spawn(move || {
        let mut log = false;
        loop {
            while let Some((path, id)) = pending.get() {
                match http::send(&path, &id, &user_data, &client) {
                    Ok(_) => pending.remove(),
                    Err(err) => {
                        warn!("{:?}", err);
                        http::health(&client);
                        log = false;
                        continue;
                    }
                };
            }

            match http::state(&user_data, &client) {
                Ok(result) => {
                    if !result {
                        match http::download(&user_data, &client) {
                            Ok(_) => (),
                            Err(err) => {
                                warn!("Failed to download up-to-date clipboard data: {}", err);
                                log = false;
                            }
                        };
                    } else {
                        if !log {
                            info!("every thihng is uptodate");
                            log = true;
                        }
                        thread::sleep(time::Duration::from_secs(3));
                    }
                }
                Err(err) => {
                    warn!("Server is down or client is offline: {:?}", err);
                    http::health(&client);
                    log = false;
                }
            }
        }
    });

    thread::spawn(move || {
        for (path, id) in rx {
            match http::send(&path, &id, &user_data1, &client1) {
                Ok(_) => (),
                Err(err) => {
                    pending1.add((id, path));
                    warn!("Failed to send recent clipboard: {}", err);
                }
            };
        }
    });
}
