use actix_multipart::Multipart;
use actix_web::HttpResponse;
use rand::seq::IteratorRandom;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashMap},
    fs::{self, File},
    io::{Error, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use zip::write::FileOptions;

pub const CRED_PATH: &str = "credentials/users";
const DATABASE_PATH: &str = "data-base/users";
const MAX_SIZE: usize = 100 * 1024 * 1024;

#[derive(Serialize, Deserialize, Debug)]
pub struct Credentials {
    username: String,
    pass: String,
}

#[derive(Debug, Clone)]
pub struct UserState {
    data: Arc<Mutex<HashMap<String, BTreeSet<String>>>>,
}

impl UserState {
    pub fn new() -> Self {
        let op = Self {
            data: Arc::new(Mutex::new(HashMap::new())),
        };

        if let Ok(users) = fs::read_dir(DATABASE_PATH) {
            for user in users.flatten() {
                if user.path().is_dir() {
                    if let Some(folder_name) = user.file_name().to_str() {
                        let mut files = BTreeSet::new();
                        if let Ok(entries) = fs::read_dir(user.path()) {
                            for entry in entries.flatten() {
                                if let Some(file_name) = entry.file_name().to_str() {
                                    files.insert(file_name.to_string());
                                }
                            }
                        }

                        let mut temp = op.data.lock().unwrap();
                        temp.insert(folder_name.to_string(), files);
                    }
                }
            }
        }
        op
    }

    pub fn entry_and_verify_user(&self, username: &str) -> bool {
        let mut map = self.data.lock().unwrap();
        if map.contains_key(username) {
            true
        } else {
            map.insert(username.to_string(), BTreeSet::new());
            false
        }
    }

    pub fn verify(&self, username: &str) -> bool {
        self.data.lock().unwrap().contains_key(username)
    }

    pub fn update(&self, username: &str, id: &str) {
        let mut map = self.data.lock().unwrap();
        if let Some(set) = map.get_mut(username) {
            set.insert(id.to_string());
        }
    }

    pub fn is_updated(&self, username: &str, id: &str) -> bool {
        self.data
            .lock()
            .unwrap()
            .get(username)
            .and_then(|set| set.iter().last())
            .map(|val| val == id)
            .unwrap_or(false)
    }

    pub fn next(&self, username: &str, id: &str) -> Result<Vec<String>, HttpResponse> {
        let map = self.data.lock().unwrap();

        let tree = map
            .get(username)
            .ok_or_else(|| HttpResponse::Unauthorized().body("Error: authentication failed"))?;

        if let Some(pos) = tree.iter().position(|x| x == id) {
            Ok(tree
                .iter()
                .skip(pos + 1)
                .map(|x| format!("{}/{}/{}", DATABASE_PATH, username, x))
                .collect())
        } else {
            Err(HttpResponse::Unauthorized().body("Error: authentication failed"))
        }
    }
}

impl Credentials {
    pub fn new(username: String, pass: String) -> Self {
        Credentials { username, pass }
    }

    pub fn write(&self) -> Result<(), std::io::Error> {
        let path = Path::new(CRED_PATH).join(&self.username).join("user.json");

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let data = serde_json::to_string_pretty(self)?;

        let mut file = fs::File::create(path)?;
        file.write_all(&data.as_bytes())?;

        Ok(())
    }

    pub fn read(user: &str) -> Result<Self, Error> {
        let path = Path::new(CRED_PATH).join(&user).join("user.json");
        let file = fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&file)?)
    }

    pub fn authentication(&self, key: String) -> bool {
        if key == self.pass { true } else { false }
    }
}

#[derive(Deserialize)]
pub struct UserCred {
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

use futures_util::StreamExt;

pub async fn write_file(
    mut data: Multipart,
    username: &str,
    id: &str,
) -> Result<(), actix_web::Error> {
    let path: PathBuf = Path::new(DATABASE_PATH).join(username).join(id);

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

    Ok(())
}

pub fn to_zip(files: Vec<String>) -> Result<HttpResponse, ()> {
    let zip_options: FileOptions<'_, ()> =
        FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    let mut buffer = Vec::new();
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buffer));

    for file in &files {
        let path = Path::new(file);
        let filename = match path.file_name() {
            Some(name) => name.to_string_lossy(),
            None => {
                eprintln!("Invalid file path: {}", file);
                continue;
            }
        };

        let mut f = match File::open(file) {
            Ok(file) => file,
            Err(err) => {
                eprintln!("{:?}", err);
                continue;
            }
        };

        zip.start_file(filename, zip_options).unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();
        zip.write_all(&buf).unwrap();
    }

    zip.finish().unwrap();
    Ok(HttpResponse::Ok().body(buffer))
}
