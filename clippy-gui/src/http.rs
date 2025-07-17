use clippy::{LoginUserCred, NewUser, NewUserOtp, UserCred, http::SERVER};
use log::debug;
use reqwest::Client;

pub async fn check_user(user: String) -> Result<bool, String> {
    let connection = Client::new();
    let data = NewUser::new(user);

    let response = connection
        .get(format!("{}/usercheck", SERVER))
        .json(&data)
        .send();

    match response.await {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<bool>().await {
                    Ok(value) => Ok(value),
                    Err(_) => Err("Invalid response format".to_string()),
                }
            } else {
                match resp.text().await {
                    Ok(msg) => Err(msg),
                    Err(_) => Err("Internal server error".to_string()),
                }
            }
        }
        Err(_) => Err("Unable to communicate with server".to_string()),
    }
}

pub async fn signin(data: NewUser) -> Result<(), String> {
    let connection = Client::new();
    let response = connection
        .post(format!("{}/signin", SERVER))
        .json(&data)
        .send();

    match response.await {
        Ok(resp) => match resp.status().as_u16() {
            200 => Ok(()),
            409 => match resp.text().await {
                Ok(v) => Err(v),
                Err(e) => {
                    debug!("{e}");
                    Err(String::from("Unable to connect to server."))
                }
            },
            _ => {
                let msg = resp.text().await.unwrap_or("Server error".to_string());
                Err(msg)
            }
        },
        Err(_) => Err("Unable to contact server".to_string()),
    }
}

pub async fn signin_otp_auth(data: NewUserOtp) -> Result<UserCred, String> {
    let connection = Client::new();
    let response = connection
        .post(format!("{}/authotp", SERVER))
        .json(&data)
        .send()
        .await;

    match response {
        Ok(res) => {
            if res.status().is_success() {
                if let Ok(user) = res.json::<UserCred>().await {
                    Ok(user)
                } else {
                    Err("Invalid server response while parsing user data".to_string())
                }
            } else {
                let body = res.text().await.unwrap_or_default();
                Err(body)
            }
        }
        Err(e) => {
            debug!("{e}");
            Err("Unable to connect to server".to_string())
        }
    }
}

pub async fn login(user: &LoginUserCred) -> Result<UserCred, String> {
    let connection = Client::new();
    let response = connection
        .get(format!("{}/login", SERVER))
        .json(&user)
        .send()
        .await
        .map_err(|err| format!("{err}"))?;

    if response.status().is_success() {
        let user: UserCred = response.json().await.map_err(|err| format!("{err}"))?;
        Ok(user)
    } else {
        let err_msg = response.text().await.map_err(|err| format!("{err}"))?;
        Err(format!("Login failed: {}", err_msg).into())
    }
}
