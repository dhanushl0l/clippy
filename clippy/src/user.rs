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

const API_KEY: Option<&str> = option_env!("KEY");

pub fn user() -> Result<UserSettings, Box<dyn Error>> {
    let mut user_config = get_path();
    user_config.pop();
    user_config.push("user");
    if !user_config.is_dir() {
        create_dir(&user_config)?;
    }

    user_config.push(".user");
    if user_config.is_file() {
        let file = fs::read(user_config)?;
        let file = decrypt_file(API_KEY.unwrap().as_bytes(), &file).unwrap();
        Ok(serde_json::from_str(&String::from_utf8(file).unwrap()).unwrap())
    } else {
        let usersettings: UserSettings = UserSettings::new();
        let file = serde_json::to_string_pretty(&usersettings)?;
        let file = encrept_file(API_KEY.unwrap().as_bytes(), file.as_bytes()).unwrap();
        fs::write(&user_config, file)?;
        Ok(usersettings)
    }
}

pub fn cloud(rx: Receiver<(String, String)>) {
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
