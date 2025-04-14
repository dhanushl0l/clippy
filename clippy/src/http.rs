use crate::{
    UserData, extract_zip,
    write_clipboard::{self},
};
use core::time;
use log::{debug, info, warn};
use reqwest::blocking::{Client, multipart};
use std::{fs::File, thread, time::Duration};

const SERVER: &str = "http://192.168.1.106:8080";

pub fn send(file_path: &str, id: &str, userdata: &UserData, client: &Client) -> Result<(), String> {
    let _file = File::open(&file_path).map_err(|e| format!("File error: {}", e))?;

    let form = multipart::Form::new()
        .file("file", &file_path)
        .map_err(|e| format!("Multipart error: {}", e))?;

    client
        .get(&format!("{}/update", SERVER))
        .query(&[("username", "d"), ("pass", "1"), ("id", &id)])
        .multipart(form)
        .send()
        .map_err(|e| format!("Request error: {}", e))?;

    info!("Sent file [{}] successfully.", id);

    userdata.add(id.to_string());

    Ok(())
}

pub fn state(userdata: &UserData, client: &Client) -> Result<bool, String> {
    let user = "d";
    let response = client
        .get(&format!("{}/state/{}", SERVER, user))
        .query(&[("id", userdata.last_one())])
        .send()
        .map_err(|e| format!("Request error: {}", e))?;

    let body = match response.text() {
        Ok(text) => text,
        Err(e) => format!("<Failed to read body: {}>", e),
    };

    match body.as_str() {
        "OUTDATED" => Ok(false),
        "UPDATED" => Ok(true),
        _ => Err("<Failed to read body: {}>".to_string()),
    }
}

pub fn download(userdata: &UserData, client: &Client) -> Result<(), String> {
    let user = "d";
    let response = client
        .get(&format!("{}/get", SERVER))
        .query(&[("username", user)])
        .query(&[("pass", "1")])
        .query(&[("current", userdata.last_one())])
        .send()
        .map_err(|e| format!("Request error: {}", e))?;

    let body = match response.bytes() {
        Ok(text) => text,
        Err(e) => return Err(format!("<Failed to read body: {}>", e)),
    };

    match extract_zip(body) {
        Ok(val) => {
            info!("Successfully fetched data from server.");
            debug!("{:?}", &val);
            userdata.add_vec(val)
        }
        Err(_) => (),
    }

    #[cfg(not(target_os = "linux"))]
    write_clipboard::copy_to_clipboard(userdata).map_err(|err| format!("{}", err))?;

    #[cfg(target_os = "linux")]
    write_clipboard::copy_to_linux(userdata).map_err(|err| format!("{}", err))?;
    Ok(())
}

pub fn health(client: &Client) {
    let mut log = true;
    loop {
        let response = client
            .get(format!("{}/health", SERVER))
            .timeout(Duration::from_secs(5))
            .send();

        match response {
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
