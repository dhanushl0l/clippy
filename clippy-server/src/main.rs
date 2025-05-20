use actix_multipart::Multipart;
use actix_web::{
    App, HttpResponse, HttpServer, Responder,
    web::{self},
};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use chrono::Utc;
use clippy::{LoginUserCred, NewUser, NewUserOtp};
use clippy_server::{
    CRED_PATH, EmailState, OTPState, UserCred, UserState, auth, gen_otp, gen_password, get_auth,
    get_param, hash_key, to_zip, write_file,
};
use email::send_otp;
use env_logger::{Builder, Env};
use log::debug;
use std::{
    collections::HashMap,
    fs::{self},
    path::Path,
};
mod email;

macro_rules! param {
    ($map:expr, $key:expr) => {
        match get_param($map, $key) {
            Ok(val) => val,
            Err(res) => return res,
        }
    };
}

async fn signin(
    data: web::Json<NewUser>,
    state: web::Data<UserState>,
    emailstate: web::Data<EmailState>,
    otp_state: web::Data<OTPState>,
) -> impl Responder {
    let username = &data.user;

    if state.verify(username) {
        return HttpResponse::Unauthorized().body("Failure: Username already exists");
    } else if emailstate.check_email(data.email.clone().unwrap()) {
        return HttpResponse::Unauthorized().body("Failure: Email already exists");
    } else {
        let otp = gen_otp();
        otp_state.add_otp(username.to_string(), otp.clone());
        match send_otp(&data, otp).await {
            Ok(_) => HttpResponse::Ok().body("SURCESS"),
            Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
        }
    }
}

async fn signin_auth(
    data: web::Json<NewUserOtp>,
    state: web::Data<UserState>,
    email_state: web::Data<EmailState>,
    otp_state: web::Data<OTPState>,
) -> impl Responder {
    let username = &data.user;

    if state.verify(username) {
        return HttpResponse::Unauthorized().body("Failure: Username already exists");
    } else {
        match otp_state.check_otp(&data) {
            Ok(_) => {
                let password = hash_key(&data.key, &data.user);
                if let Err(err) =
                    UserCred::new(username.clone(), data.email.clone(), password).write()
                {
                    eprintln!("Failure: failed to write credentials\n{}", err);
                    return HttpResponse::InternalServerError()
                        .body("Error: Failed to write credentials");
                }

                email_state.add(data.email.clone());

                let file_path = Path::new(CRED_PATH).join(username).join("user.json");
                match fs::read_to_string(&file_path) {
                    Ok(content) => {
                        if let Some(_) = state.entry_and_verify_user(username) {
                            println!("{:?}", state);
                            HttpResponse::Ok()
                                .content_type("application/json")
                                .body(content)
                        } else {
                            HttpResponse::Unauthorized().body("Failure: Username already exists")
                        }
                    }
                    Err(err) => {
                        eprintln!("Error reading user file: {}", err);
                        HttpResponse::NotFound().body("Error: JSON file not found")
                    }
                }
            }
            Err(err) => return HttpResponse::Unauthorized().body(err),
        }
    }
}

async fn check_user(state: web::Data<UserState>, data: web::Json<NewUser>) -> impl Responder {
    let username = &data.user;

    if state.verify(username) {
        HttpResponse::Ok().json(true)
    } else {
        HttpResponse::Ok().json(false)
    }
}

async fn get_key(cred: web::Json<UserCred>, state: web::Data<UserState>) -> impl Responder {
    if state.verify(&cred.username) {
        let user_cred_db = match UserCred::read(&cred.username) {
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
    } else {
        HttpResponse::Unauthorized().body("User credentials do not match")
    }
}

async fn login(cred: web::Json<LoginUserCred>, state: web::Data<UserState>) -> impl Responder {
    if state.verify(&cred.username) {
        let user_cred_db = match UserCred::read(&cred.username) {
            Ok(val) => {
                println!("{:?}", state);
                val
            }
            Err(err) => {
                return HttpResponse::Unauthorized()
                    .body(format!("User not found: {}", err.to_string()));
            }
        };
        if user_cred_db.verify(&cred) {
            HttpResponse::Ok().json(user_cred_db)
        } else {
            HttpResponse::Unauthorized().body("User credentials do not match")
        }
    } else {
        HttpResponse::Unauthorized().body("User credentials do not match")
    }
}

async fn update(
    payload: Multipart,
    auth_key: BearerAuth,
    state: web::Data<UserState>,
) -> impl Responder {
    let key = auth_key.token();
    let id = Utc::now().timestamp();

    let username = match auth(key) {
        Ok(val) => val,
        Err(err) => return HttpResponse::Unauthorized().body(err.to_string()),
    };

    match write_file(payload, &username, id.to_string()).await {
        Ok(_) => {
            debug!("writing {}/{}", username, id)
        }
        Err(err) => {
            return err.error_response();
        }
    }
    state.update(&username, id);

    HttpResponse::Ok().body(id.to_string())
}

async fn state(
    user: web::Path<String>,
    id: web::Query<HashMap<String, String>>,
    state: web::Data<UserState>,
) -> impl Responder {
    let user = user.into_inner();
    let id = param!(&id, "id");

    if state.is_updated(&user, &id) {
        HttpResponse::Ok().body("UPDATED")
    } else {
        HttpResponse::Ok().body("OUTDATED")
    }
}

async fn get(
    auth_key: BearerAuth,
    current: web::Query<HashMap<String, String>>,
    state: web::Data<UserState>,
) -> impl Responder {
    let key = auth_key.token();
    let current = param!(&current, "current");

    let username = match auth(key) {
        Ok(val) => val,
        Err(err) => return HttpResponse::Unauthorized().body(err.to_string()),
    };

    let files = match state.next(&username, &current) {
        Ok(val) => val,
        Err(err) => return err,
    };
    match to_zip(files) {
        Ok(data) => data,
        Err(err) => {
            eprintln!("{:?}", err);
            HttpResponse::Unauthorized().body(err.to_string())
        }
    }
}

async fn health() -> impl Responder {
    HttpResponse::Ok()
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    Builder::from_env(Env::default().filter_or("LOG", "info")).init();

    // Instead of reading the email and user separately,implemented a single method where userstate::build creates and builds both the UserState and EmailState
    let temp = UserState::build();
    let user_state = web::Data::new(temp.0);
    let email_state = web::Data::new(temp.1);
    let otp_state = web::Data::new(OTPState::new());

    HttpServer::new(move || {
        App::new()
            .app_data(user_state.clone())
            .app_data(otp_state.clone())
            .app_data(email_state.clone())
            .route("/state/{user}", web::get().to(state))
            .route("/update", web::post().to(update))
            .route("/signin", web::post().to(signin))
            .route("/authotp", web::post().to(signin_auth))
            .route("/login", web::get().to(login))
            .route("/get", web::get().to(get))
            .route("/getkey", web::get().to(get_key))
            .route("/usercheck", web::get().to(check_user))
            .route("/health", web::get().to(health))
    })
    .bind(("0.0.0.0", 7777))?
    .run()
    .await
}
