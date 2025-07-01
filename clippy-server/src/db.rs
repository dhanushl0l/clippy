use chrono::{Duration, Local, NaiveDateTime};
use clippy::{NewUser, NewUserOtp};
use sqlx::{Error, Pool, Postgres, Row, query, query_as};

use crate::{CustomErr, UserCred};

pub async fn get_user(pool: &Pool<Postgres>, username: &str) -> Result<UserCred, Error> {
    let user =
        query_as::<_, UserCred>("SELECT username, email, key FROM usercred WHERE username = $1")
            .bind(username)
            .persistent(false)
            .fetch_one(pool)
            .await?;

    Ok(user)
}

pub async fn is_user_exists(pool: &Pool<Postgres>, username: &str) -> Result<bool, Error> {
    let exists = query(
        r#"
    SELECT EXISTS (
        SELECT 1 FROM usercred WHERE username = $1
    )
    "#,
    )
    .bind(username)
    .persistent(false)
    .fetch_one(pool)
    .await?;

    Ok(exists.get("exists"))
}

pub async fn is_email_exists(pool: &Pool<Postgres>, email: &str) -> Result<bool, Error> {
    let exists = query(
        r#"
    SELECT EXISTS (
        SELECT 1 FROM usercred WHERE email = $1
    )
    "#,
    )
    .bind(email)
    .persistent(false)
    .fetch_one(pool)
    .await?;

    Ok(exists.get("exists"))
}

pub async fn add_otp(user: &NewUser, otp: String, pool: &Pool<Postgres>) -> Result<(), Error> {
    query(
        "INSERT INTO otp_state (email, username, otp, attempt, time_created)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (email)
         DO UPDATE SET username = $2, otp = $3, attempt = $4, time_created = $5",
    )
    .bind(user.email.clone().unwrap())
    .bind(user.user.clone())
    .bind(otp)
    .bind(0)
    .bind(Local::now().naive_local())
    .persistent(false)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn check_otp(user: &NewUserOtp, pool: &Pool<Postgres>) -> Result<(), CustomErr> {
    let row = query("SELECT attempt, otp, username, time_created FROM otp_state WHERE email = $1")
        .bind(user.email.clone())
        .persistent(false)
        .fetch_one(pool)
        .await;

    let row = match row {
        Ok(va) => va,
        Err(er) => return Err(CustomErr::DBError(er)),
    };

    let attempt: i32 = row.get("attempt");
    if attempt >= 4 {
        return Err(CustomErr::Failed("OTP attempt expired".to_string()));
    }

    let otp: String = row.get("otp");
    if otp != user.otp {
        if let Err(e) = increment_attempt(pool, &user.email).await {
            return Err(CustomErr::DBError(e));
        };
        return Err(CustomErr::Failed(
            "Invalid OTP. Please check your registered email for the correct code.".to_string(),
        ));
    }

    let username: String = row.get("username");
    if username != user.user {
        return Err(CustomErr::Failed("Invalid username".to_string()));
    }

    let now = Local::now().naive_local();
    let time: NaiveDateTime = row.get("time_created");
    if now - time > Duration::minutes(15) {
        return Err(CustomErr::Failed("OTP expired".to_string()));
    }

    Ok(())
}

pub async fn increment_attempt(pool: &Pool<Postgres>, email: &str) -> Result<(), Error> {
    query("UPDATE otp_state SET attempt = attempt + 1 WHERE email = $1")
        .bind(email)
        .persistent(false)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn write(user: &UserCred, pool: &Pool<Postgres>) -> Result<(), Error> {
    let _ = query("INSERT INTO usercred (username, email, key) VALUES ($1, $2, $3)")
        .bind(user.username.clone())
        .bind(user.email.clone())
        .bind(user.key.clone())
        .persistent(false)
        .execute(pool)
        .await?;

    Ok(())
}
