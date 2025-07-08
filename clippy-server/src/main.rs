mod db;
mod email;

use std::env;

use crate::{
    db::{add_otp, check_otp, get_user, is_email_exists, is_user_exists},
    email::send_otp,
};
use actix_web::{
    App, HttpRequest, HttpResponse, HttpServer, Responder,
    web::{self},
};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use clippy::{
    LoginUserCred, NewUser, NewUserOtp, is_valid_email, is_valid_otp, is_valid_password,
    is_valid_username,
};
use clippy_server::{
    CustomErr, DB_CONF, RoomManager, SECRET_KEY, SMTP_PASSWORD, SMTP_USERNAME, UserCred, UserState,
    auth, gen_otp, get_auth, get_oncelock, hash_key,
};
use env_logger::{Builder, Env};
use log::{debug, error};
use sqlx::{PgPool, Pool, Postgres};

async fn signin(new_user: web::Json<NewUser>, pool: web::Data<Pool<Postgres>>) -> impl Responder {
    if new_user
        .email
        .as_ref()
        .map_or(true, |va| !is_valid_email(va))
        || !is_valid_username(&new_user.user)
    {
        return HttpResponse::Unauthorized().body("Failure: Invalid credentials");
    }

    match is_user_exists(pool.as_ref(), &new_user.user).await {
        Ok(true) => HttpResponse::Conflict().body("Failure: Username already exists"),
        Ok(false) => match is_email_exists(pool.as_ref(), &new_user.email.clone().unwrap()).await {
            Ok(true) => HttpResponse::Conflict().body("Failure: Email already exists"),
            Ok(false) => {
                let otp = gen_otp();
                if let Err(e) = add_otp(&new_user, otp.clone(), pool.as_ref()).await {
                    debug!("{:?}", e);
                    return HttpResponse::InternalServerError().body("Unable to retreve otp");
                };
                match send_otp(&new_user, otp).await {
                    Ok(_) => HttpResponse::Ok().body("SURCESS"),
                    Err(e) => {
                        debug!("{}", e);
                        HttpResponse::InternalServerError().body("Unable to check status")
                    }
                }
            }
            Err(e) => {
                debug!("{}", e);
                HttpResponse::InternalServerError().body("Unable to check status")
            }
        },
        Err(e) => {
            debug!("{}", e);
            HttpResponse::InternalServerError().body("Unable to check status")
        }
    }
}

async fn signin_auth(
    data: web::Json<NewUserOtp>,
    pool: web::Data<Pool<Postgres>>,
) -> impl Responder {
    let username = &data.user;
    if !is_valid_username(&data.user)
        || !is_valid_email(&data.email)
        || !is_valid_password(&data.key)
        || !is_valid_otp(&data.otp)
    {
        return HttpResponse::Unauthorized().body("Failure: Invalid credentials");
    }

    match check_otp(&data, pool.as_ref()).await {
        Ok(_) => {
            let password = hash_key(&data.key, &data.user);
            let user = UserCred::new(username.clone(), data.email.clone(), password);
            if let Err(err) = db::write(&user, pool.as_ref()).await {
                error!("Failure: failed to write credentials\n{}", err);
                return HttpResponse::InternalServerError()
                    .body("Error: Failed to write credentials");
            }

            HttpResponse::Ok()
                .content_type("application/json")
                .body(serde_json::to_string(&user).unwrap())
        }
        Err(err) => match err {
            CustomErr::DBError(err) => {
                debug!("{}", err);
                return HttpResponse::Unauthorized().body("Unable to veriify otp");
            }
            CustomErr::Failed(er) => return HttpResponse::Unauthorized().body(er),
        },
    }
}

async fn check_user(data: web::Json<NewUser>, pool: web::Data<Pool<Postgres>>) -> impl Responder {
    let username = &data.user;
    if !is_valid_username(username) {
        return HttpResponse::Unauthorized().body("Failure: Invalid credentials");
    }

    match is_user_exists(pool.as_ref(), username).await {
        Ok(val) => HttpResponse::Ok().json(val),
        Err(err) => {
            debug!("{}", err);
            HttpResponse::InternalServerError().body("Unable to get proper response")
        }
    }
}

async fn get_key(cred: web::Json<UserCred>, pool: web::Data<Pool<Postgres>>) -> impl Responder {
    if !is_valid_username(&cred.username) {
        return HttpResponse::Unauthorized()
            .body("Failure: Invalid credentials, Try updating the app");
    }
    let user_cred_db = match get_user(pool.as_ref(), &cred.username).await {
        Ok(val) => val,
        Err(err) => {
            return HttpResponse::Unauthorized()
                .body(format!("User not found: {}", err.to_string()));
        }
    };
    if user_cred_db == *cred {
        let key = match get_auth(&cred.username, 1) {
            Ok(val) => val,
            Err(err) => return HttpResponse::InternalServerError().body(err.to_string()),
        };
        HttpResponse::Ok().body(key)
    } else {
        HttpResponse::Unauthorized().body("User credentials do not match")
    }
}

async fn login(cred: web::Json<LoginUserCred>, pool: web::Data<Pool<Postgres>>) -> impl Responder {
    if !is_valid_username(&cred.username) || !is_valid_password(&cred.key) {
        return HttpResponse::Unauthorized().body("Failure: Invalid credentials");
    }
    let user_cred_db = match get_user(pool.get_ref(), &cred.username).await {
        Ok(val) => val,
        Err(_) => return HttpResponse::Unauthorized().body("Failure: Invalid credentials"),
    };

    if user_cred_db.verify(&cred) {
        HttpResponse::Ok().json(user_cred_db)
    } else {
        HttpResponse::Unauthorized().body("Failure: Invalid credentials")
    }
}

async fn health() -> impl Responder {
    HttpResponse::Ok().body("SERVER_ACTIVE")
}

async fn handle_connection(
    auth_key: BearerAuth,
    req: HttpRequest,
    stream: web::Payload,
    room: web::Data<RoomManager>,
    state: web::Data<UserState>,
) -> Result<HttpResponse, actix_web::Error> {
    let key = auth_key.token();
    let username = match auth(key) {
        Ok(val) => val,
        Err(err) => {
            return {
                error!("unable to process jwt: {}", err);
                Err(actix_web::error::ErrorUnauthorized("Unable to authorize."))
            };
        }
    };
    state
        .entry(&username)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    let (res, session, msg_stream) = actix_ws::handle(&req, stream)?;
    let msg_stream = msg_stream
        .max_frame_size(30 * 1024 * 1024)
        .aggregate_continuations()
        .max_continuation_size(30 * 1024 * 1024);

    room.add_task(username, session, msg_stream, room.clone(), state.clone())
        .await;
    Ok(res)
}

pub fn init_env() {
    SMTP_USERNAME
        .set(env::var("SMTP_USERNAME").expect("SMTP_USERNAME not set"))
        .ok();
    SMTP_PASSWORD
        .set(env::var("SMTP_PASSWORD").expect("SMTP_PASSWORD not set"))
        .ok();
    SECRET_KEY.set(env::var("KEY").expect("KEY not set")).ok();
    DB_CONF.set(env::var("DB_CONF").expect("KEY not set")).ok();
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    Builder::from_env(Env::default().filter_or("LOG", "info")).init();
    init_env();

    let user_state = web::Data::new(UserState::new());
    let room = web::Data::new(RoomManager::new());
    let pool = web::Data::new(PgPool::connect(get_oncelock(&DB_CONF)).await.unwrap());

    HttpServer::new(move || {
        App::new()
            .app_data(user_state.clone())
            .app_data(room.clone())
            .app_data(pool.clone())
            .route("/connect", web::get().to(handle_connection))
            .route("/signin", web::post().to(signin))
            .route("/authotp", web::post().to(signin_auth))
            .route("/login", web::get().to(login))
            .route("/getkey", web::get().to(get_key))
            .route("/usercheck", web::get().to(check_user))
            .route("/health", web::get().to(health))
    })
    .bind(("0.0.0.0", 7777))?
    .run()
    .await
}
