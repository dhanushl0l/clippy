use crate::{
    UserCred, UserData, UserSettings, extract_zip, read_data_by_id, set_global_update_bool,
    write_clipboard::{self},
};
use core::time;
use log::{debug, error, warn};
use once_cell::sync::Lazy;
use reqwest::{self, Client, multipart};
use std::{
    error::{self, Error},
    process,
    sync::Mutex,
    thread,
    time::Duration,
};
use tokio::{fs::File, io::AsyncReadExt};

#[cfg(debug_assertions)]
pub const SERVER: &str = "http://127.0.0.1:7777"; // debug build

#[cfg(not(debug_assertions))]
pub const SERVER: &str = "https://clippy.dhanu.cloud"; // release build

static TOKEN: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new(String::new()));

pub fn update_token(new_data: String) {
    let mut key = TOKEN.lock().unwrap();
    *key = new_data;
}

fn get_token() -> String {
    let key = TOKEN.lock().unwrap();
    key.clone()
}

pub async fn send_to_cloud(
    file_path: &str,
    usercred: &UserCred,
    client: &Client,
    userdata: &UserData,
    last: bool,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    if last {
        match just_download(userdata, client).await {
            Ok(_) => (),
            Err(e) => {
                debug!("{}", e);
            }
        };
    }
    let result = send(&file_path, &usercred, &client).await;

    result
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
            user.write().unwrap();
            error!("Unable to verify credentials, logging out.");
            process::exit(1);
        }
        let err_msg = response.text().await?;
        Err(format!("Login failed: {}", err_msg).into())
    }
}

pub async fn state(userdata: &UserData, client: &Client, user: &UserCred) -> Result<bool, String> {
    let response = client
        .get(&format!("{}/state", SERVER))
        .bearer_auth(get_token())
        .query(&[("id", userdata.last_one())])
        .send()
        .await
        .map_err(|e| format!("Request error: {}", e))?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        warn!("Token expired");
        match get_token_serv(user, client).await {
            Ok(_) => debug!("Fetched a new authentication token"),
            Err(err) => {
                warn!("Unable to fetch authentication token");
                debug!("{}", err);
            }
        };
        return Err(String::from("Token expired"));
    }

    let body = match response.text().await {
        Ok(text) => text,
        Err(e) => format!("Failed to read body: {}", e),
    };

    match body.as_str() {
        "OUTDATED" => Ok(false),
        "UPDATED" => Ok(true),
        _ => Err("Failed to read body: {}".to_string()),
    }
}

pub async fn download(userdata: &UserData, client: &Client) -> Result<(), Box<dyn error::Error>> {
    let data = userdata.get_30();
    let response = client
        .get(&format!("{}/get", SERVER))
        .bearer_auth(get_token())
        .json(&data)
        .send()
        .await?;

    let body = if response.status().is_success() {
        response.bytes().await?
    } else {
        if response.status() == 401 {
            get_token();
            return Err(format!("Auth token expired").into());
        }
        return Err(format!("{}", response.status()).into());
    };

    set_global_update_bool(true);

    let val = extract_zip(body)?;
    if !val.is_sorted() {
        panic!("val is not sorted make it sorted fn name clippy/http download()")
    }
    if let Some(last) = val.last() {
        let data = read_data_by_id(last);

        match data {
            Ok(val) => {
                #[cfg(not(target_os = "linux"))]
                write_clipboard::copy_to_clipboard(val).unwrap();

                #[cfg(target_os = "linux")]
                write_clipboard::copy_to_linux(val)?;
            }
            Err(err) => {
                warn!("{}", err)
            }
        }
    } else {
        error!("Unable to read last value in tar")
    }

    userdata.add_vec(val);

    Ok(())
}

pub async fn just_download(userdata: &UserData, client: &Client) -> Result<(), reqwest::Error> {
    let data = userdata.get_30();
    let response = client
        .get(&format!("{}/get", SERVER))
        .bearer_auth(get_token())
        .json(&data)
        .send()
        .await?;

    match response.status() {
        reqwest::StatusCode::UNAUTHORIZED => {
            get_token();
            Err(response.error_for_status().unwrap_err())
        }
        reqwest::StatusCode::ALREADY_REPORTED => {
            debug!("No need for data fetch, DB updated");
            Ok(())
        }
        reqwest::StatusCode::OK => {
            let body = response.error_for_status()?.bytes().await?;
            let val = extract_zip(body).unwrap();
            userdata.add_vec(val);
            set_global_update_bool(true);
            Ok(())
        }
        _ => {
            return Err(response.error_for_status().unwrap_err());
        }
    }
}

pub async fn health(client: &Client) {
    let mut log = true;
    loop {
        let response = client
            .get(format!("{}/health", SERVER))
            .timeout(Duration::from_secs(5))
            .send();

        match response.await {
            Ok(response) => {
                if response.status().is_success() {
                    break;
                } else {
                    if log {
                        warn!("Server is out");
                        log = false
                    }
                }
            }
            Err(err) => {
                debug!("{}", err);
                thread::sleep(time::Duration::from_secs(5));
            }
        }
    }
}
