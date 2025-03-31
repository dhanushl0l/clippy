use reqwest::blocking::{Client, multipart};
use std::fs::File;

use crate::{UserData, extract_zip};

const SERVER: &str = "http://127.0.0.1:8080";

pub fn send(file_path: String, id: String, userdata: &UserData) -> Result<(), String> {
    match state(userdata) {
        Ok(true) => (),
        Ok(false) => {
            let log = download(userdata);
            println!("{:?}", log)
        }
        Err(err) => eprintln!("{:?}", err),
    }

    let client = Client::new();

    let _file = File::open(&file_path).map_err(|e| format!("File error: {}", e))?;

    let form = multipart::Form::new()
        .file("file", &file_path)
        .map_err(|e| format!("Multipart error: {}", e))?;

    let response = client
        .get(&format!("{}/update", SERVER))
        .query(&[("username", "d"), ("pass", "1"), ("id", &id)])
        .multipart(form)
        .send()
        .map_err(|e| format!("Request error: {}", e))?;

    println!("Status: {}", response.status());
    let body = response
        .text()
        .unwrap_or_else(|_| "<Failed to read body>".to_string());

    userdata.add(id);
    println!("Body: {}", body);

    Ok(())
}

pub fn state(userdata: &UserData) -> Result<bool, String> {
    let user = "d";
    let client = Client::new();
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

pub fn download(userdata: &UserData) -> Result<(), String> {
    let user = "d";
    let client = Client::new();
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
            println!("{:?}", &val);
            userdata.add_vec(val)
        }
        Err(_) => (),
    }

    Ok(())
}
