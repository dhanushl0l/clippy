use clippy::NewUser;
use clippy_server::{SMTP_PASSWORD, SMTP_USERNAME, get_oncelock};
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use std::error::Error;

pub async fn send_otp(user: &NewUser, otp: String) -> Result<(), Box<dyn Error>> {
    let email = Message::builder()
        .from(format!("Clippy <{}>", get_oncelock(&SMTP_USERNAME)).parse()?)
        .to(format!("{} <{}>", user.user, user.email.as_ref().unwrap()).parse()?)
        .subject("Welcome to Clippy Community – Here's Your OTP")
        .header(ContentType::TEXT_PLAIN)
        .body(format!(
            "Hey there!\n\nWelcome to the Clippy community! 🎉\nWe're thrilled to have you on board.\n\nHere’s your one-time password (OTP): **{}**\n\nDon’t worry, we won’t make you memorize it forever — it’s only valid for a short time.\n\nIf you didn’t request this, just ignore it.\n\nCheers,\nTeam Clippy",
            otp
        ))?;

    let creds = Credentials::new(
        get_oncelock(&SMTP_USERNAME).to_string(),
        get_oncelock(&SMTP_PASSWORD).to_string(),
    );

    let mailer: AsyncSmtpTransport<Tokio1Executor> =
        AsyncSmtpTransport::<Tokio1Executor>::relay("smtp.gmail.com")?
            .credentials(creds)
            .build();

    mailer.send(email).await?;
    Ok(())
}
