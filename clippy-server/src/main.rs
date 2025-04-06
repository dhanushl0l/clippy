use actix_multipart::Multipart;
use clippy_server::{
    CRED_PATH, Credentials, UserState, gen_password, get_param, to_zip, write_file,
};
use std::{
    collections::HashMap,
    fs::{self},
    path::Path,
};

use actix_web::{
    App, HttpResponse, HttpServer, Responder,
    web::{self},
};

macro_rules! param {
    ($map:expr, $key:expr) => {
        match get_param($map, $key) {
            Ok(val) => val,
            Err(res) => return res,
        }
    };
}

async fn signin(
    username: web::Query<HashMap<String, String>>,
    state: web::Data<UserState>,
) -> impl Responder {
    let username = param!(&username, "username");

    if state.entry_and_verify_user(&username) {
        return HttpResponse::Unauthorized().body("Failure: Username already exist");
    } else {
        let pass = gen_password();
        match Credentials::new(username.clone(), pass).write() {
            Ok(_) => println!("new user added"),
            Err(err) => eprintln!("Failure: failed to write credentials\n{}", err),
        };
        let file_path = Path::new(CRED_PATH).join(&username).join("user.json");
        match fs::read_to_string(file_path) {
            Ok(content) => HttpResponse::Ok()
                .content_type("application/json")
                .body(content),
            Err(_) => HttpResponse::NotFound().body("Error: JSON file not found"),
        }
    }
}

async fn update(
    user: web::Query<HashMap<String, String>>,
    key: web::Query<HashMap<String, String>>,
    id: web::Query<HashMap<String, String>>,
    payload: Multipart,
    state: web::Data<UserState>,
) -> impl Responder {
    let username = param!(&user, "username");
    let key = param!(&key, "pass");
    let id = param!(&id, "id");

    if state.verify(&username) {
        let user = match Credentials::read(&username) {
            Ok(val) => val,
            Err(_) => {
                return HttpResponse::Unauthorized().body("Error: authentication failed");
            }
        };

        if user.authentication(key) {
            match write_file(payload, &username, &id).await {
                Ok(_) => (),
                Err(err) => {
                    let response = err.error_response();
                    return response;
                }
            }
            state.update(&username, &id);
        } else {
            return HttpResponse::Unauthorized().body("Error: authentication failed");
        }
    } else {
        return HttpResponse::Unauthorized().body("Error: authentication failed");
    }

    HttpResponse::Ok().body("SURCESS")
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
    user: web::Query<HashMap<String, String>>,
    key: web::Query<HashMap<String, String>>,
    current: web::Query<HashMap<String, String>>,
    state: web::Data<UserState>,
) -> impl Responder {
    let username = param!(&user, "username");
    let key = param!(&key, "pass");
    let current = param!(&current, "current");

    if state.verify(&username) {
        let user = match Credentials::read(&username) {
            Ok(val) => val,
            Err(_) => {
                return HttpResponse::Unauthorized().body("Error: authentication failed");
            }
        };

        if user.authentication(key) {
            let files = match state.next(&username, &current) {
                Ok(val) => val,
                Err(err) => return err,
            };
            match to_zip(files) {
                Ok(data) => data,
                Err(err) => HttpResponse::Unauthorized().body("Error: authentication failed"),
            }
        } else {
            return HttpResponse::Unauthorized().body("Error: authentication failed");
        }
    } else {
        return HttpResponse::Unauthorized().body("Error: authentication failed");
    }
}

async fn health() -> impl Responder {
    HttpResponse::Ok()
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let user_state = web::Data::new(UserState::new());

    HttpServer::new(move || {
        App::new()
            .app_data(user_state.clone())
            .route("/state/{user}", web::get().to(state))
            .route("/update", web::get().to(update))
            .route("/signin", web::get().to(signin))
            .route("/get", web::get().to(get))
            .route("/health", web::get().to(health))
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
