use crate::{MessageChannel, UserCred, UserData, UserSettings};
use core::time;
use log::{debug, error, warn};
use once_cell::sync::Lazy;
use reqwest::{self, Client, multipart};
use std::{error::Error, process, sync::Mutex, thread, time::Duration};
use tokio::{fs::File, io::AsyncReadExt, sync::mpsc::Receiver};

#[cfg(debug_assertions)]
pub const SERVER: &str = "http://192.168.1.240:7777";

#[cfg(not(debug_assertions))]
pub const SERVER: &str = "https://clippy.dhanu.cloud";

#[cfg(debug_assertions)]
pub const SERVER_WS: &str = "ws://192.168.1.240:7777/connect";

#[cfg(not(debug_assertions))]
pub const SERVER_WS: &str = "wss://clippy.dhanu.cloud/connect";

static TOKEN: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new(String::new()));

pub fn update_token(new_data: String) {
    let mut key = TOKEN.lock().unwrap();
    *key = new_data;
}

pub fn get_token() -> String {
    let key = TOKEN.lock().unwrap();
    key.clone()
}

pub async fn send(
    file_path: &str,
    usercred: &UserCred,
    client: &Client,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mut file = File::open(file_path).await?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).await?;

    let part = multipart::Part::bytes(buffer);
    let form = multipart::Form::new().part("file", part);

    let response = client
        .post(&format!("{}/update", SERVER))
        .bearer_auth(get_token())
        .multipart(form)
        .send()
        .await?;

    if response.status().is_success() {
        Ok(response.text().await?)
    } else {
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            warn!("Token expired");

            match get_token_serv(usercred, client).await {
                Ok(_) => debug!("Fetched a new authentication token"),
                Err(err) => {
                    warn!("Unable to fetch authentication token");
                    debug!("{}", err);
                }
            }
        }
        Err(response.text().await?.into())
    }
}

pub async fn get_token_serv(user: &UserCred, client: &Client) -> Result<(), Box<dyn Error>> {
    let response = client
        .get(format!("{}/getkey", SERVER))
        .json(&user)
        .send()
        .await?;

    if response.status().is_success() {
        let token = response.text().await?;
        update_token(token);
        Ok(())
    } else {
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            let mut user = UserSettings::build_user().unwrap();
            user.remove_user();
            user.write_local().unwrap();
            error!(
                "Unable to verify credentials, logging out. {:?}",
                response.text().await
            );
            process::exit(1);
        }
        let err_msg = response.text().await?;
        Err(format!("Login failed: {}", err_msg).into())
    }
}

pub async fn health(
    client: &Client,
    rx: &mut Receiver<MessageChannel>,
    user_data: &UserData,
) -> bool {
    let mut log = true;
    loop {
        let response = client
            .get(format!("{}/health", SERVER))
            .timeout(Duration::from_secs(5))
            .send();

        match response.await {
            Ok(response) => {
                if response.status().is_success() && response.status().as_u16() == 200 {
                    if let Ok(text) = response.text().await {
                        if text == "SERVER_ACTIVE" {
                            break false;
                        }
                    } else {
                        warn!("Server is out");
                        thread::sleep(time::Duration::from_secs(5));
                    }
                } else {
                    if log {
                        warn!("Server is out");
                        log = false
                    }
                    thread::sleep(time::Duration::from_secs(5));
                }
            }
            Err(err) => {
                debug!("ubale to connect :{:?}|{}", client, err);
                thread::sleep(time::Duration::from_secs(5));
            }
        }
        while let Ok(val) = rx.try_recv() {
            match val {
                MessageChannel::New { path, time, typ } => {
                    user_data
                        .add_pending(
                            time,
                            crate::Edit::New {
                                path: path.into(),
                                typ,
                            },
                        )
                        .await;
                }
                MessageChannel::Edit {
                    old_id,
                    new_id,
                    typ,
                    path,
                } => {
                    user_data
                        .add_pending(
                            old_id,
                            crate::Edit::Edit {
                                path: path.into(),
                                typ,
                                new_id,
                            },
                        )
                        .await;
                }
                MessageChannel::SettingsChanged => {
                    unimplemented!("to do")
                }
                MessageChannel::Remove(id) => {
                    user_data.add_pending(id, crate::Edit::Remove).await;
                }
            }
        }
    }
}
