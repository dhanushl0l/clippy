use clippy::{LoginUserCred, NewUser, NewUserOtp, UserCred, http::SERVER};
use reqwest::{Client, Error};

pub async fn check_user(user: String) -> Option<bool> {
    let connection = Client::new();
    let data = NewUser::new(user);

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

pub async fn signin(data: NewUser) -> Result<bool, Error> {
    let connection = Client::new();
    let response = connection
        .post(format!("{}/signin", SERVER))
        .json(&data)
        .send()
        .await?;

    if response.status().is_success() {
        Ok(true)
    } else {
        Ok(false)
    }
}

pub async fn signin_otp_auth(data: NewUserOtp) -> Result<UserCred, Error> {
    let connection = Client::new();
    let response = connection
        .post(format!("{}/authotp", SERVER))
        .json(&data)
        .send()
        .await?;

    Ok(response.json().await?)
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
