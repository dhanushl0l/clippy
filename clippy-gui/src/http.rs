use std::error::Error;

use clippy::{UserCred, Username, http::SERVER};
use reqwest::blocking::Client;

pub fn check_user(user: String) -> Option<bool> {
    let connection = Client::new();
    let data = Username { user };

    let response = connection
        .post(format!("{}/usercheck", SERVER))
        .json(&data)
        .send();

    match response {
        Ok(resp) => match resp.json::<bool>() {
            Ok(value) => Some(value),
            Err(_) => None,
        },
        Err(_) => None,
    }
}

pub fn signin(username: String) -> Result<UserCred, Box<dyn Error>> {
    let connection = Client::new();
    let data = Username::new(username);
    let response = connection
        .post(format!("{}/signin", SERVER))
        .json(&data)
        .send()?;

    let user: UserCred = response.json()?;
    Ok(user)
}

pub fn login(user: UserCred) -> Result<(), Box<dyn Error>> {
    let connection = Client::new();
    let response = connection
        .post(format!("{}/login", SERVER))
        .json(&user)
        .send()?;

    if response.status().is_success() {
        Ok(())
    } else {
        let err_msg = response.text()?;
        Err(format!("Login failed: {}", err_msg).into())
    }
}
