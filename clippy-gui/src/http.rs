use core::fmt;
use std::io;

use clippy::{UserCred, Username, http::SERVER};
use reqwest::{Client, Error};

pub async fn check_user(user: String) -> Option<bool> {
    let connection = Client::new();
    let data = Username { user };

    let response = connection
        .get(format!("{}/usercheck", SERVER))
        .json(&data)
        .send();

    match response.await {
        Ok(resp) => match resp.json::<bool>().await {
            Ok(value) => Some(value),
            Err(_) => None,
        },
        Err(_) => None,
    }
}

pub async fn signin(username: String) -> Result<UserCred, Error> {
    let connection = Client::new();
    let data = Username::new(username);
    let response = connection
        .post(format!("{}/signin", SERVER))
        .json(&data)
        .send()
        .await?;

    let user: UserCred = response.json().await?;
    Ok(user)
}

pub async fn login(user: UserCred) -> Result<Option<UserCred>, Error> {
    let connection = Client::new();
    let response = connection
        .get(format!("{}/getkey", SERVER))
        .json(&user)
        .send()
        .await?;

    if response.status().is_success() {
        let text = response.text().await?;
        println!("{:?}", text);
        Ok(Some(user))
    } else {
        Ok(None)
    }
}
