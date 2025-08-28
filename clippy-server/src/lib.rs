mod ws_connection;
use actix_multipart::Multipart;
use actix_web::{HttpResponse, rt, web};
use actix_ws::{AggregatedMessageStream, Session};
use base64::{Engine, engine::general_purpose};
use chrono::{Duration, Utc};
use clippy::{LoginUserCred, NewUserOtp};
use futures_util::StreamExt;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use log::{debug, error};
use rand::seq::IteratorRandom;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::prelude::FromRow;
use std::{
    collections::{BTreeSet, HashMap, VecDeque, hash_map::Entry},
    error::Error,
    fs::{self},
    io::{self, Write},
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};
use tokio::sync::{self, broadcast::Sender};
use ws_connection::ws_connection;

pub const DATABASE_PATH: &str = "data-base/users";
const MAX_SIZE: usize = 10 * 1024 * 1024;
const MAX_COUNT: usize = 100;
pub static SMTP_USERNAME: OnceLock<String> = OnceLock::new();
pub static SMTP_PASSWORD: OnceLock<String> = OnceLock::new();
pub static SECRET_KEY: OnceLock<String> = OnceLock::new();
pub static DB_CONF: OnceLock<String> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct UserState {
    data: Arc<Mutex<HashMap<String, User>>>,
}

#[derive(Debug, Clone)]
pub struct User {
    state: BTreeSet<String>,
    remove: VecDeque<String>,
}

impl UserState {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn remove_and_add_edit(&self, username: &str, remove: &str) -> Result<(), String> {
        let mut map = self.data.lock().map_err(|_| "Mutex poisoned")?;
        let user = map
            .get_mut(username)
            .ok_or("unable to identify user".to_string())?;

        user.state.remove(remove);
        println!("remove {}", remove);
        match remove_db_file(username, remove) {
            Ok(_) => (),
            Err(err) => debug!("{}", err),
        };

        if user.remove.len() >= MAX_COUNT {
            user.remove.pop_front();
        }
        user.remove.push_back(remove.to_string());

        Ok(())
    }

    pub fn entry(&self, username: &str) -> Result<(), String> {
        let mut map = self.data.lock().map_err(|_| "Mutex poisoned")?;

        let dir_path = format!("{}/{}", DATABASE_PATH, username);
        fs::create_dir_all(&dir_path)
            .map_err(|e| format!("Failed to create dir {}: {}", dir_path, e))?;

        if !map.contains_key(username) {
            let user = User {
                state: BTreeSet::new(),
                remove: VecDeque::new(),
            };
            map.insert(username.to_string(), user);
            debug!("{:?}", self);
        }

        Ok(())
    }

    pub fn verify(&self, username: &str) -> bool {
        self.data.lock().unwrap().contains_key(username)
    }

    pub fn update(&self, username: &str, id: &str) {
        let mut map = self.data.lock().unwrap();
        if let Some(set) = map.get_mut(username) {
            let len = set.state.len();
            if len > 29 {
                let remove_count = len - 29;
                let to_remove: Vec<_> = set.state.iter().take(remove_count).cloned().collect();
                for val in to_remove {
                    debug!("removing {:?}", val);
                    match remove_db_file(username, &val) {
                        Ok(_) => (),
                        Err(err) => error!("{}", err),
                    };
                    set.state.remove(&val);
                }
            }

            set.state.insert(id.to_string());
        } else {
            error!("Unabe to update user state: {}", id);
        }
    }

    pub fn is_updated(&self, username: &str, id: &str) -> bool {
        let guard = match self.data.lock() {
            Ok(g) => g,
            Err(e) => {
                error!("Mutex lock failed: {}", e);
                panic!("Mutex lock failed")
            }
        };

        let data = guard.get(username);
        if let Some(val) = data {
            match val.state.last() {
                Some(last) => {
                    debug!("{},{}", id, last);
                    last == id
                }
                None => {
                    error!("User '{}' has empty BTreeSet", username);
                    false
                }
            }
        } else {
            true
        }
    }

    pub fn next(&self, username: &str, id: &str) -> Result<Vec<String>, HttpResponse> {
        let map = self.data.lock().unwrap();

        let tree = map
            .get(username)
            .ok_or_else(|| HttpResponse::Unauthorized().body("Error: authentication failed"))?;

        if let Some(pos) = tree.state.iter().position(|x| x == id) {
            Ok(tree
                .state
                .iter()
                .skip(pos + 1)
                .map(|x| format!("{}/{}/{}", DATABASE_PATH, username, x))
                .collect())
        } else {
            if !map.is_empty() {
                Ok(tree
                    .state
                    .iter()
                    .map(|x| format!("{}/{}/{}", DATABASE_PATH, username, x))
                    .collect())
            } else {
                Err(HttpResponse::InternalServerError().body("Error: failed to get data position"))
            }
        }
    }

    pub fn get(&self, username: &str, id: &[String]) -> Option<Vec<(String, String)>> {
        let map = self.data.lock().unwrap();
        let tree = map.get(username)?;
        let mut temp = Vec::new();
        for i in tree.state.iter() {
            if !id.contains(i) {
                temp.push((
                    format!("{}/{}/{}", DATABASE_PATH, username, i),
                    i.to_string(),
                ));
            }
        }

        Some(temp)
    }

    pub fn get_remove(&self, username: &str) -> VecDeque<String> {
        let map = self.data.lock().unwrap();
        let tree = map.get(username).unwrap();
        tree.remove.clone()
    }

    pub fn remove(&self, username: &str, id: &str) -> Result<(), Box<dyn Error>> {
        let mut map = self.data.lock().or_else(|_| Err("Unable to lock cred"))?;
        let tree = map
            .get_mut(username)
            .ok_or_else(|| "Unable to identify user")?;
        if tree.state.remove(id) {
            match remove_db_file(username, id) {
                Ok(_) => (),
                Err(err) => debug!("{}", err),
            };
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct EmailState {
    data: Arc<Mutex<Vec<String>>>,
}

impl EmailState {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn check_email(&self, email: String) -> bool {
        self.data.lock().unwrap().contains(&email)
    }

    pub fn add(&self, email: String) {
        self.data.lock().unwrap().push(email);
    }
}

pub enum CustomErr {
    DBError(sqlx::Error),
    Failed(String),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, FromRow)]
pub struct UserCred {
    pub username: String,
    pub email: String,
    pub key: String,
}

impl UserCred {
    pub fn new(username: String, email: String, key: String) -> Self {
        Self {
            username,
            email,
            key,
        }
    }

    pub fn authentication(&self, key: String) -> bool {
        if key == self.key { true } else { false }
    }

    pub fn verify(&self, logincred: &LoginUserCred) -> bool {
        self.username == logincred.username
            && self.key == hash_key(&logincred.key, &logincred.username)
    }
}

#[derive(Deserialize)]
pub struct UserCredState {
    pub username: String,
    pub key: String,
    pub id: String,
}

pub fn get_param(map: &HashMap<String, String>, key: &str) -> Result<String, HttpResponse> {
    map.get(key)
        .cloned()
        .ok_or_else(|| HttpResponse::Unauthorized().body("not found"))
}

pub fn gen_password() -> String {
    let mut rng = rand::rng();
    let len = 12;
    let possible_chars: Vec<char> = "qwertyuiopasdfghjklzxcvbnm1234567890".chars().collect();
    let mut password = String::with_capacity(len);

    for _ in 0..len {
        if let Some(&random) = possible_chars.iter().choose(&mut rng) {
            password.push(random);
        }
    }

    password
}

pub fn gen_otp() -> String {
    let mut rng = rand::rng();
    let len = 6;
    let possible_chars: Vec<char> = ('0'..='9').collect();
    let mut otp = String::with_capacity(len);

    for _ in 0..len {
        if let Some(&random) = possible_chars.iter().choose(&mut rng) {
            otp.push(random);
        }
    }

    otp
}

pub async fn write_file(
    mut data: Multipart,
    username: &str,
    id: i64,
) -> Result<String, actix_web::Error> {
    let mut path: PathBuf = PathBuf::new().join(format!("{}/{}", DATABASE_PATH, username));

    let file_name = get_filename(id, path.clone());
    path.push(&file_name);

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(actix_web::error::ErrorInternalServerError)?;
    }

    let mut file = fs::File::create(&path).map_err(actix_web::error::ErrorInternalServerError)?;

    let mut total_size: usize = 0;
    while let Some(field) = data.next().await {
        if let Ok(mut field) = field {
            while let Some(chunk) = field.next().await {
                let data = chunk.unwrap();
                total_size += data.len();

                if total_size > MAX_SIZE {
                    drop(file);
                    let _ = fs::remove_file(&path);
                    return Err(actix_web::error::ErrorBadRequest("File exceeds 10MB limit"));
                }
                file.write_all(&data)?;
            }
        }
    }

    Ok(file_name)
}

pub fn get_filename(id: i64, mut path: PathBuf) -> String {
    let mut file_name = id.to_string();
    let mut cont = 10;
    file_name.push('-');
    file_name.push_str(&cont.to_string());

    path.push(&file_name);

    let len = file_name.len();

    while path.exists() {
        cont += 1;
        path.pop();
        file_name.truncate(len - 2);
        file_name.push_str(&cont.to_string());
        path.push(&file_name);
    }
    file_name
}

#[derive(Deserialize)]
pub struct Claims {
    user: String,
}

pub fn auth(key: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_audience(&["clippy"]);
    validation.set_issuer(&["https://clippy.dhanu.cloud"]);

    let token = decode::<Claims>(
        &key,
        &DecodingKey::from_secret(get_oncelock(&SECRET_KEY).as_ref()),
        &validation,
    )?;

    Ok(token.claims.user.to_string())
}

pub struct OTPState {
    data: Arc<Mutex<HashMap<String, (String, i64, u32)>>>,
}

impl OTPState {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    pub fn add_otp(&self, user: String, otp: String) {
        let now = Utc::now();
        let expiry_time = now + Duration::minutes(5);

        self.data
            .lock()
            .unwrap()
            .insert(user, (otp, expiry_time.timestamp(), 0));

        self.remove_expired();
    }

    pub fn remove_expired(&self) {
        let now = Utc::now().timestamp();
        let mut map = self.data.lock().unwrap();
        map.retain(|_, (_, expiry, _)| *expiry > now);
    }

    pub fn check_otp(&self, user_otp: &NewUserOtp) -> Result<(), String> {
        if let Some((val, exp, attempt)) = self.data.lock().unwrap().get_mut(&user_otp.user) {
            if *attempt > 4 {
                return Err(String::from(
                    "You have exceeded the maximum number of attempts.",
                ));
            }

            if *val != user_otp.otp {
                *attempt += 1;
                return Err(String::from("Invalid otp"));
            }

            if is_token_expired(*exp) {
                return Err(String::from(
                    "This code has expired. Please request a new one.",
                ));
            }
        } else {
            return Err(String::from("Invalid User"));
        };
        Ok(())
    }
}

pub struct RoomManager {
    room: sync::Mutex<HashMap<String, Room>>,
}
#[derive(Debug)]
pub struct Room {
    clients: sync::Mutex<Vec<rt::task::JoinHandle<()>>>,
    tx: Sender<MessageMPC>,
}

impl RoomManager {
    pub fn new() -> Self {
        Self {
            room: sync::Mutex::new(HashMap::new()),
        }
    }

    pub async fn add_task(
        &self,
        user: String,
        session: Session,
        msg_stream: AggregatedMessageStream,
        room: web::Data<RoomManager>,
        state: actix_web::web::Data<UserState>,
    ) {
        let mut rooms = self.room.lock().await;
        match rooms.entry(user.clone()) {
            Entry::Occupied(mut entry) => {
                let a = entry.get_mut();
                let tx = a.tx.clone();
                a.add(session, msg_stream, tx, state, user, room).await;
            }
            Entry::Vacant(entry) => {
                let mut new_room = Room::new();
                let tx = new_room.tx.clone();
                new_room
                    .add(session, msg_stream, tx, state, user, room)
                    .await;
                entry.insert(new_room);
            }
        }
    }

    pub async fn remove(&self, user: String, pos: usize) {
        let rooms = self.room.lock().await;
        if let Some(room) = &mut rooms.get(&user) {
            let mut coll = room.clients.lock().await;
            coll.remove(pos);
        }
    }

    pub async fn remove_inactive(&self) {
        let mut room = self.room.lock().await;
        let mut remove_room = Vec::new();
        for (k, v) in room.iter_mut() {
            let client = v.clients.get_mut();
            if client.is_empty() {
                remove_room.push(k.clone());
            } else {
                client.retain(|x| !x.is_finished());
            }
        }
        for i in remove_room {
            room.remove(&i);
        }
    }
}

impl Room {
    fn new() -> Self {
        let (tx, _) = tokio::sync::broadcast::channel(100);
        Self {
            clients: sync::Mutex::new(Vec::new()),
            tx,
        }
    }

    async fn add(
        &mut self,
        session: Session,
        msg_stream: AggregatedMessageStream,
        tx: Sender<MessageMPC>,
        state: actix_web::web::Data<UserState>,
        user: String,
        room: web::Data<RoomManager>,
    ) {
        let val = self.clients.get_mut();
        val.push(rt::spawn(ws_connection(
            session, msg_stream, tx, state, user, room,
        )));
        debug!("total threads {}", val.len());
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MessageMPC {
    New(String),
    Edit { old_id: String, new_id: String },
    Remove(String),
    None,
}
pub fn get_auth(username: &str, exp: i64) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let time = now.to_rfc3339();
    let expiry_time = now + Duration::hours(exp);

    let claims = json!({
        "iss": "https://clippy.dhanu.cloud",
        "aud": "clippy",
        "user": username,
        "iat": now.timestamp(),
        "exp": expiry_time.timestamp(),
        "created": time
    });

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(get_oncelock(&SECRET_KEY).as_ref()),
    )?;

    Ok(token)
}

fn is_token_expired(expiry_timestamp: i64) -> bool {
    let now_timestamp = Utc::now().timestamp();
    now_timestamp >= expiry_timestamp
}

fn remove_db_file(username: &str, id: &str) -> Result<(), io::Error> {
    let mut path = PathBuf::from(DATABASE_PATH);
    path.push(username);
    path.push(id);
    debug!("remove path {:?}", &path);
    fs::remove_file(path)?;
    Ok(())
}

pub fn hash_key(key: &str, user: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key);
    hasher.update(user);
    let result = hasher.finalize();
    general_purpose::STANDARD.encode(result)
}

pub fn get_oncelock(key: &OnceLock<String>) -> &str {
    key.get().expect("not initialized")
}
