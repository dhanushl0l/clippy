pub mod encryption_decryption;
pub mod http;
pub mod macros;
pub mod read_clipboard;
pub mod user;
pub mod write_clipboard;

use base64::Engine;
use base64::engine::general_purpose;
use bytes::Bytes;
use bytestring::ByteString;
use chrono::prelude::Utc;
use encryption_decryption::{decrypt_file, encrept_file};
use image::{ImageReader, load_from_memory};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::error::Error;
use std::fs::create_dir;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use std::{
    collections::BTreeSet,
    env,
    fs::File,
    fs::{self},
    io::{self},
    path::PathBuf,
};
use std::{process, thread};
use tar::{Archive, Builder};
use tokio::sync::Notify;
use tokio::sync::mpsc::Sender;

pub const APP_ID: &str = "org.clippy.clippy";
pub const API_KEY: Option<&str> = option_env!("KEY");
pub static SETTINGS: Mutex<Option<UserSettings>> = Mutex::new(None);
const IMAGE_DATA: &[u8] = include_bytes!("../../assets/gui_icons/image.png");

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Data {
    data: String,
    pub typ: String,
    device: String,
    pub pined: bool,
}

impl Data {
    pub fn new(data: String, typ: String, device: String, pined: bool) -> Self {
        Data {
            data,
            typ,
            device,
            pined,
        }
    }

    pub fn write_to_json(&self, tx: &Sender<(String, String, String)>) -> Result<(), io::Error> {
        let time = Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string();
        let path = get_path_pending();
        fs::create_dir_all(&path)?;
        let file_path = &path.join(&time);

        let store_image = match SETTINGS.lock() {
            Ok(guard) => guard.as_ref().map_or(true, |va| va.store_image),
            Err(_) => true,
        };

        if self.typ.starts_with("image/") && store_image {
            if let Err(e) = self.save_image(&time) {
                error!("Unable to write thumbnail");
                debug!("{e}")
            };
        }

        let json_data = serde_json::to_string_pretty(self)?;

        let mut file = File::create(file_path)?;
        file.write_all(json_data.as_bytes())?;

        set_global_update_bool(true);

        match tx.try_send((file_path.to_str().unwrap().into(), time, self.typ.clone())) {
            Ok(_) => (),
            Err(err) => warn!(
                "Failed to send file '{}' to channel: {}",
                file_path.display(),
                err
            ),
        }

        Ok(())
    }

    pub fn get_data(&self) -> Option<String> {
        if !self.typ.starts_with("image/") {
            Some(self.data.clone())
        } else {
            None
        }
    }

    pub fn get_image_thumbnail(&self, id: &PathBuf) -> Option<(Vec<u8>, (u32, u32))> {
        let path = get_image_path(id);
        let image = if path.is_file() {
            ImageReader::open(path).ok()?.decode().ok()?
        } else {
            load_from_memory(IMAGE_DATA).ok().unwrap()
        };
        let rgba = image.to_rgba8();

        let size = (rgba.width(), rgba.height());
        Some((rgba.into_raw(), size))
    }

    pub fn get_image(&self) -> Option<Vec<u8>> {
        if self.typ.starts_with("image/") {
            Some(general_purpose::STANDARD.decode(&self.data).unwrap())
        } else {
            None
        }
    }

    pub fn get_image_as_string(&self) -> Option<&str> {
        if self.typ.starts_with("image/") {
            Some(&self.data)
        } else {
            None
        }
    }

    pub fn get_pined(&self) -> bool {
        self.pined
    }

    pub fn change_pined(&mut self) {
        self.pined = !self.pined
    }

    pub fn change_data(&mut self, data: &str) {
        self.data = data.to_string()
    }

    pub fn save_image(&self, time: &str) -> Result<(), io::Error> {
        let path: PathBuf = crate::get_path_image();

        fs::create_dir_all(&path)?;

        let img_path = path.join(format!("{}.png", time));
        let mut img_file = File::create(img_path)?;

        let data: Vec<u8> = self
            .get_image()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Failed to get image data"))?;

        let image = image::load_from_memory(&data).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Image decode error: {e}"),
            )
        })?;

        let resized = image.thumbnail(128, 128);

        resized
            .write_to(&mut img_file, image::ImageFormat::Png)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Image write error: {e}")))?;

        Ok(())
    }

    pub fn get_meta_data(&self) -> Option<String> {
        let Some(data) = self.get_data() else {
            return Some(String::new());
        };

        let lines = data.lines().take(10).map(|line| {
            let line = line.trim();
            if line.len() > 100 {
                format!("{}..", &line[..70])
            } else {
                line.trim().to_string()
            }
        });

        let display_text = lines.collect::<Vec<_>>().join("\n");

        Some(display_text)
    }
}

#[derive(Debug, Clone)]
pub struct UserData {
    data: Arc<Mutex<BTreeSet<String>>>,
}

impl UserData {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(BTreeSet::new())),
        }
    }

    pub fn build() -> Self {
        let mut temp = BTreeSet::new();

        fs::create_dir_all(&get_path()).unwrap();

        let folder_path = &get_path();

        if let Ok(entries) = fs::read_dir(folder_path.as_path()) {
            for entry in entries.flatten() {
                if let Some(file_name) = entry.file_name().to_str() {
                    temp.insert(file_name.to_string());
                }
            }
        }

        debug!("{:?}", temp);

        Self {
            data: Arc::new(Mutex::new(temp)),
        }
    }

    pub fn add(&self, id: String, total: Option<u32>) {
        let mut data = self.data.lock().unwrap();
        data.insert(id);
        debug!("User clipboard count: {}", data.len());

        if let Some(val) = total {
            let len = data.len() as u32;
            if val < len {
                let count = len - val;
                let to_remove: Vec<String> = data.iter().take(count as usize).cloned().collect();
                let mut path = get_path();
                let mut pined_path = get_path_pined();
                for i in to_remove {
                    path.push(&i);
                    let file = File::open(&path).unwrap();
                    let clipboard: Data = serde_json::from_reader(file).unwrap();
                    if clipboard.pined {
                        pined_path.push(&i);
                        fs::rename(&path, &pined_path).unwrap();
                        pined_path.pop();
                    } else {
                        log_error!(fs::remove_file(&path));
                    }
                    path.pop();
                    data.remove(&i);
                }
                debug!("User clipboard count: {}", data.len());
            }
        }
    }

    pub fn get_30(&self) -> Vec<String> {
        self.data
            .lock()
            .unwrap()
            .iter()
            .rev()
            .take(30)
            .cloned()
            .collect()
    }

    pub fn add_vec(&self, id: Vec<String>) {
        for id in id {
            self.data.lock().unwrap().insert(id);
        }
    }

    pub fn last_one(&self) -> String {
        self.data
            .lock()
            .unwrap()
            .last()
            .unwrap_or(&"".to_string())
            .clone()
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct UserCred {
    pub username: String,
    pub email: String,
    pub key: String,
}

impl UserCred {
    pub fn new(username: String, key: String, email: String) -> Self {
        Self {
            username,
            key,
            email,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct LoginUserCred {
    pub username: String,
    pub key: String,
}

impl LoginUserCred {
    pub fn new(username: String, key: String) -> Self {
        Self { username, key }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct UserSettings {
    sync: Option<UserCred>,
    pub store_image: bool,
    pub click_on_quit: bool,
    pub paste_on_click: bool,
    pub disable_sync: bool,
    encrept: Option<String>,
    pub intrevel: u32,
    pub max_clipboard: Option<u32>,
    pub theme: SystemTheam,
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub enum SystemTheam {
    System,
    Dark,
    Light,
}

impl UserSettings {
    pub fn new() -> Self {
        Self {
            sync: None,
            disable_sync: false,
            store_image: true,
            encrept: None,
            click_on_quit: true,
            paste_on_click: true,
            intrevel: 3,
            max_clipboard: Some(100),
            theme: SystemTheam::System,
        }
    }

    pub fn remove_user(&mut self) {
        self.sync = None;
    }

    pub fn get_sync(&self) -> &Option<UserCred> {
        &self.sync
    }

    pub fn set_user(&mut self, val: UserCred) {
        self.sync = Some(val)
    }

    pub fn is_login(&self) -> bool {
        !(self.sync == None)
    }

    pub fn build_user() -> Result<Self, Box<dyn Error>> {
        let mut user_config = get_path_local();
        user_config.push("user");
        if !user_config.is_dir() {
            create_dir(&user_config)?;
        }
        user_config.push(".settings");
        let file = fs::read(&user_config)?;
        let mut settings: UserSettings = serde_json::from_slice(&file).unwrap();
        user_config.pop();
        user_config.push(".user");
        let file = if let Ok(data) = fs::read(&user_config) {
            Some(data)
        } else {
            None
        };
        if let Some(file) = file {
            match decrypt_file(API_KEY.unwrap().as_bytes(), &file) {
                Ok(va) => {
                    let data: UserCred =
                        serde_json::from_str(&String::from_utf8(va).unwrap()).unwrap();
                    settings.sync = Some(data);
                }
                Err(e) => {
                    error!("Unable to get read user");
                    debug!("{}", e);
                    fs::remove_file(user_config)?;
                }
            };
        }
        if let Ok(mut va) = SETTINGS.lock() {
            *va = Some(settings.clone());
        }
        Ok(settings)
    }

    pub fn write(&self) -> Result<(), Box<dyn Error>> {
        let mut user_config = get_path_local();
        user_config.push("user");
        user_config.push(".user");
        let user = self.sync.clone();
        if let Some(data) = user {
            let data = serde_json::to_vec_pretty(&data)?;
            let en_data = encrept_file(API_KEY.unwrap().as_bytes(), &data)
                .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
            let mut file = File::create(&user_config)?;
            file.write_all(&en_data)?;
        } else {
            if user_config.is_file() {
                if let Err(e) = fs::remove_file(&user_config) {
                    error!("Unable to store settings");
                    debug!("{}", e);
                };
            }
        }

        user_config.pop();
        user_config.push(".settings");

        let mut data = self.clone();
        data.sync = None;
        let data = serde_json::to_vec_pretty(&data)?;
        let mut file = File::create(&user_config)?;
        file.write_all(&data)?;

        Ok(())
    }
}

#[derive(PartialEq, Debug)]
pub enum DataState {
    WaitingToSend,
    SentButNotAcked,
}

#[derive(Debug)]
pub struct Value {
    path: PathBuf,
    typ: String,
    state: DataState,
}

impl Value {
    fn new(path: PathBuf, typ: String) -> Self {
        Self {
            path,
            typ,
            state: DataState::WaitingToSend,
        }
    }
}

#[derive(Debug)]
pub struct Pending {
    data: BTreeMap<String, Value>,
    notify: Arc<Notify>,
}

impl Pending {
    pub fn new() -> Self {
        Self {
            data: BTreeMap::new(),
            notify: Arc::new(Notify::new()),
        }
    }

    pub fn build() -> Result<Self, io::Error> {
        let mut temp = BTreeMap::new();
        for entry in fs::read_dir(get_path_pending())? {
            let path: PathBuf = entry?.path();

            let file_content = fs::read_to_string(&path)?;
            let data: Data = serde_json::from_str(&file_content)?;
            let file_name = if let Some(name) = path.file_name().and_then(|f| f.to_str()) {
                name.to_string()
            } else {
                continue;
            };
            temp.insert(file_name, Value::new(path, data.typ));
        }
        Ok(Self {
            data: temp,
            notify: Arc::new(Notify::new()),
        })
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub async fn next(&self) -> Option<(bool, String, &Value)> {
        loop {
            let mut found: Option<(&String, &Value)> = None;
            let mut count = 0;

            for (k, v) in &self.data {
                if v.state == DataState::WaitingToSend {
                    count += 1;
                    if found.is_none() {
                        found = Some((k, v));
                    } else {
                        break;
                    }
                }
            }

            if let Some((k, v)) = found {
                let is_last = count == 1;
                return Some((is_last, k.clone(), v));
            }

            self.notify.notified().await;
        }
    }

    pub fn add(&mut self, path: String, id: String, typ: String) {
        self.notify.notify_one();
        self.data.insert(id, Value::new(PathBuf::from(path), typ));
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&Value> {
        self.data.get(id)
    }

    pub fn remove(&mut self, id: &str) -> Option<Value> {
        self.data.remove(id)
    }

    pub fn empty(&mut self) {
        self.data.clear();
    }

    pub fn get_zip(&self) -> Result<(Vec<u8>, usize), ()> {
        let mut buffer = Vec::new();
        {
            let mut tar = Builder::new(&mut buffer);

            for (id, value) in self.data.iter() {
                let path = &value.path;
                if !path.is_file() {
                    warn!("Unable to locate {}", id);
                    continue;
                }
                tar.append_path_with_name(path, path.file_name().unwrap())
                    .unwrap();
            }
            tar.finish().unwrap();
        }

        Ok((buffer, self.len()))
    }

    pub fn pop(&mut self, id: &str) {
        self.data.remove(id);
    }

    pub fn get_pending(&self) -> Vec<(&String, &Value)> {
        let mut temp = Vec::new();
        for (id, value) in self.data.iter() {
            temp.push((id, value));
        }
        temp
    }

    pub fn change_state(&mut self, id: &str) {
        let temp = self.data.get_mut(id);
        if let Some(val) = temp {
            val.state = DataState::SentButNotAcked;
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NewUser {
    pub user: String,
    pub email: Option<String>,
}

impl NewUser {
    pub fn new(user: String) -> Self {
        Self { user, email: None }
    }

    pub fn new_signin(user: String, email: String) -> Self {
        Self {
            user,
            email: Some(email),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct NewUserOtp {
    pub user: String,
    pub email: String,
    pub otp: String,
    pub key: String,
}

impl NewUserOtp {
    pub fn new(user: String, email: String, otp: String, key: String) -> Self {
        Self {
            user,
            email,
            otp,
            key,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub enum Resopnse {
    Updated,
    Outdated,
    CheckVersion(String),
    CheckVersionArr(Vec<String>),
    Success {
        old: String,
        new: String,
    },
    Error(String),
    Data {
        data: String,
        id: String,
        last: bool,
    },
}

impl Resopnse {
    pub fn to_bytestring(&self) -> Result<ByteString, serde_json::Error> {
        let json = serde_json::to_string(self)?;
        Ok(ByteString::from(json))
    }
}

pub enum MessageType {
    Text,
    Binary,
}

pub fn get_path_local() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let path: PathBuf = [home.as_str(), ".local/share/clippy/"].iter().collect();
        fs::create_dir_all(&path).unwrap();
        return path;
    }

    #[cfg(target_os = "macos")]
    {
        let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let path: PathBuf = [home.as_str(), ".local/share/clippy/"].iter().collect();
        fs::create_dir_all(&path).unwrap();
        return path;
    }

    #[cfg(target_os = "windows")]
    {
        let home = env::var("APPDATA").unwrap_or_else(|_| "C:\\Users\\Public".to_string());
        let path: PathBuf = [home.as_str(), "clippy"].iter().collect();
        fs::create_dir_all(&path).unwrap();
        return path;
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        compile_error!("Unsupported operating system");
    }
}

pub fn get_path() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let path: PathBuf = [home.as_str(), ".local/share/clippy/data"].iter().collect();
        fs::create_dir_all(&path).unwrap();
        return path;
    }

    #[cfg(target_os = "macos")]
    {
        let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let path: PathBuf = [home.as_str(), ".local/share/clippy/data"].iter().collect();
        fs::create_dir_all(&path).unwrap();
        return path;
    }

    #[cfg(target_os = "windows")]
    {
        let home = env::var("APPDATA").unwrap_or_else(|_| "C:\\Users\\Public\\AppData".to_string());
        let path: PathBuf = [home.as_str(), "clippy\\data"].iter().collect();
        fs::create_dir_all(&path).unwrap();
        return path;
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        compile_error!("Unsupported operating system");
    }
}

pub fn get_path_pending() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let path: PathBuf = [home.as_str(), ".local/share/clippy/local_data"]
            .iter()
            .collect();
        fs::create_dir_all(&path).unwrap();
        return path;
    }

    #[cfg(target_os = "macos")]
    {
        let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let path: PathBuf = [home.as_str(), ".local/share/clippy/local_data"]
            .iter()
            .collect();
        fs::create_dir_all(&path).unwrap();
        return path;
    }

    #[cfg(target_os = "windows")]
    {
        let home = env::var("APPDATA").unwrap_or_else(|_| "C:\\Users\\Public\\AppData".to_string());
        let path: PathBuf = [home.as_str(), "clippy\\local_data"].iter().collect();
        fs::create_dir_all(&path).unwrap();
        return path;
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        compile_error!("Unsupported operating system");
    }
}

pub fn get_path_image() -> PathBuf {
    let path: PathBuf = {
        #[cfg(target_os = "linux")]
        {
            let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            [home.as_str(), ".local/share/clippy/image"]
                .iter()
                .collect()
        }

        #[cfg(target_os = "macos")]
        {
            let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            [home.as_str(), ".local/share/clippy/image"]
                .iter()
                .collect()
        }

        #[cfg(target_os = "windows")]
        {
            let home =
                env::var("APPDATA").unwrap_or_else(|_| "C:\\Users\\Public\\AppData".to_string());
            [home.as_str(), "clippy\\image"].iter().collect()
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            compile_error!("Unsupported operating system");
        }
    };
    fs::create_dir_all(&path).unwrap();
    path
}

pub fn get_path_pined() -> PathBuf {
    let path: PathBuf = {
        #[cfg(target_os = "linux")]
        {
            let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            [home.as_str(), ".local/share/clippy/pined"]
                .iter()
                .collect()
        }

        #[cfg(target_os = "macos")]
        {
            let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            [home.as_str(), ".local/share/clippy/pined"]
                .iter()
                .collect()
        }

        #[cfg(target_os = "windows")]
        {
            let home =
                env::var("APPDATA").unwrap_or_else(|_| "C:\\Users\\Public\\AppData".to_string());
            [home.as_str(), "clippy\\pined"].iter().collect()
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            compile_error!("Unsupported operating system");
        }
    };
    fs::create_dir_all(&path).unwrap();
    path
}

pub fn cache_path() -> PathBuf {
    let base: PathBuf = {
        #[cfg(target_os = "linux")]
        {
            PathBuf::from(
                env::var("XDG_CACHE_HOME")
                    .unwrap_or_else(|_| format!("{}/.cache", env::var("HOME").unwrap())),
            )
        }

        #[cfg(target_os = "windows")]
        {
            PathBuf::from(env::var("LOCALAPPDATA").expect("LOCALAPPDATA not set"))
        }

        #[cfg(target_os = "macos")]
        {
            PathBuf::from(format!("{}/Library/Caches", env::var("HOME").unwrap()))
        }

        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            panic!("Unsupported platform");
        }
    };

    let path = base.join("clippy");
    fs::create_dir_all(&path).unwrap();
    path
}

pub fn extract_zip(data: Bytes) -> Result<Vec<String>, Box<dyn Error>> {
    let target_dir = get_path();
    let mut id = Vec::new();
    let reader = std::io::Cursor::new(data);
    let mut archive = Archive::new(reader);

    debug!("Extracting into {}", target_dir.display());

    for (i, entry) in archive.entries()?.enumerate() {
        debug!("Processing entry index: {}", i);
        let mut file = match entry {
            Ok(f) => f,
            Err(e) => {
                error!("Failed to read entry {}: {}", i, e);
                continue;
            }
        };

        let path = match file.path() {
            Ok(p) => p.to_path_buf(),
            Err(e) => {
                error!("Invalid path at entry {}: {}", i, e);
                continue;
            }
        };

        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => {
                error!("Invalid file name at entry {}: {:?}", i, path);
                continue;
            }
        };

        let mut out_path = target_dir.clone();
        out_path.push(&name);
        id.push(name.clone());

        if let Some(parent) = out_path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                error!(
                    "Directory creation failed for {}: {}",
                    out_path.display(),
                    e
                );
                continue;
            }
        }

        match File::create(&out_path) {
            Ok(mut out_file) => {
                if let Err(e) = std::io::copy(&mut file, &mut out_file) {
                    error!("Copy failed for {}: {}", out_path.display(), e);
                    continue;
                }
            }
            Err(e) => {
                error!("File creation failed for {}: {}", out_path.display(), e);
                continue;
            }
        }

        info!("Extracted file: {}", out_path.display());
    }

    if let Err(e) = store_image(&id, target_dir.clone()) {
        error!("store_image failed: {}", e);
    }

    info!("Extraction done, total files: {}", id.len());
    Ok(id)
}

pub fn store_image(id: &[String], target_dir: PathBuf) -> Result<(), Box<dyn Error>> {
    for i in id {
        let mut path = target_dir.clone();
        path.push(i);

        let file = fs::read_to_string(path)?;
        let data: Data = serde_json::from_str(&file)?;

        if data.typ.starts_with("image/") {
            data.save_image(i)?;
        }
    }
    Ok(())
}

pub fn get_image_path(id: &PathBuf) -> PathBuf {
    let mut path = get_path_image();
    let file_nema = format!("{}.png", id.file_name().unwrap().to_str().unwrap());
    path.push(file_nema);
    path
}

pub fn set_global_bool(value: bool) {
    let path = get_path_local();
    if let Err(e) = fs::create_dir_all(path.parent().unwrap()) {
        error!("Failed to create directories: {}", e);
        return;
    }

    let path = Path::new(&path).join("OK");

    if value {
        if let Err(e) = fs::File::create(&path) {
            error!("Failed to create state file: {}", e);
        }
    } else {
        if let Err(e) = fs::remove_file(&path) {
            error!("Failed to delete state file: {}", e);
        }
    }
}

// This tell the gui to refresh the db
pub fn get_global_bool() -> bool {
    let path = get_path_local();
    let path = Path::new(&path).join("OK");
    !path.exists()
}

pub fn set_global_update_bool(value: bool) {
    let mut path = get_path_local();
    if let Err(e) = fs::create_dir_all(path.parent().unwrap()) {
        error!("Failed to create directories: {}", e);
        return;
    }
    path.push("UPDATE");

    if value {
        if let Err(e) = fs::File::create(&path) {
            error!("Failed to create updated state file: {}", e);
        }
    } else {
        if let Err(e) = fs::remove_file(&path) {
            error!("Failed to updated delete state file: {}", e);
        }
    }
}

pub fn get_global_update_bool() -> bool {
    let mut path = get_path_local();
    path.push("UPDATE");
    path.exists()
}

pub fn create_past_lock(path: &PathBuf) -> Result<(), io::Error> {
    let mut dir = get_path_local();
    fs::create_dir_all(&dir)?;
    dir.push(".next");
    let mut file = File::create(&dir)?;

    file.write_all(path.to_str().unwrap().as_bytes())?;
    Ok(())
}

pub fn watch_for_next_clip_write(dir: PathBuf, paste_on_click: bool) {
    let mut target = dir.clone();
    target.push(".next");

    let mut settings = dir;
    settings.push("user");
    settings.push(".settings");

    if !settings.is_file() {
        log_error!(UserSettings::new().write());
    }

    let last_modified = fs::metadata(&settings)
        .and_then(|meta| meta.modified())
        .unwrap();

    loop {
        if fs::metadata(&target).is_ok() {
            match read_parse(&target, paste_on_click) {
                Ok(_) => {
                    info!("New item copied")
                }
                Err(err) => error!("{}", err),
            }
        }

        check_settings_updated(Path::new(&settings), last_modified);

        thread::sleep(Duration::from_secs(1));
    }

    fn check_settings_updated(path: &Path, last_modified: SystemTime) {
        match fs::metadata(path) {
            Ok(meta) => {
                if !meta.is_file() {
                    warn!("Settings updated (not a file)");
                    process::exit(0);
                }
                if let Ok(modified) = meta.modified() {
                    if modified > last_modified {
                        warn!("Settings updated (modified)");
                        process::exit(0);
                    }
                }
            }
            Err(_) => {
                warn!("Settings updated (deleted or missing)");
                process::exit(0);
            }
        }
    }

    fn read_parse(target: &PathBuf, paste_on_click: bool) -> Result<(), String> {
        let contents = fs::read_to_string(&target)
            .map_err(|e| format!("Failed to read file {:?}: {}", target, e))?;

        let data = serde_json::from_str(&fs::read_to_string(&contents).unwrap()).unwrap();

        #[cfg(target_os = "linux")]
        copy_to_linux(data, paste_on_click);

        #[cfg(not(target_os = "linux"))]
        write_clipboard::push_to_clipboard(data, paste_on_click).unwrap();

        fs::remove_file(&target).map_err(|e| format!("Failed to remove {:?}: {}", target, e))?;
        Ok(())
    }
}

#[cfg(target_os = "linux")]
pub fn copy_to_linux(data: Data, paste_on_click: bool) {
    use write_clipboard::{push_to_clipboard, push_to_clipboard_wl};

    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        log_error!(push_to_clipboard_wl(data, false, paste_on_click));
    } else if std::env::var("DISPLAY").is_ok() {
        log_error!(push_to_clipboard(data, paste_on_click));
    }
}

pub fn read_data_by_id(id: &str) -> Result<Data, io::Error> {
    let mut path = get_path();
    path.push(id);

    let data_str = fs::read_to_string(path)?;

    let data: Data = serde_json::from_str(&data_str)?;

    Ok(data)
}

pub fn is_valid_username(username: &str) -> bool {
    let len_ok = username.len() >= 3 && username.len() <= 20;
    let chars_ok = username
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    len_ok && chars_ok
}

pub fn is_valid_password(password: &str) -> bool {
    let len = password.len();
    if len <= 6 || len >= 32 {
        return false;
    }

    let mut has_upper = false;
    let mut has_lower = false;
    let mut has_digit = false;
    let mut has_symbol = false;

    for c in password.chars() {
        if c.is_ascii_uppercase() {
            has_upper = true;
        } else if c.is_ascii_lowercase() {
            has_lower = true;
        } else if c.is_ascii_digit() {
            has_digit = true;
        } else if c.is_ascii_punctuation() || c.is_ascii_graphic() && !c.is_alphanumeric() {
            has_symbol = true;
        }
    }

    has_upper && has_lower && has_digit && has_symbol
}

pub fn is_valid_email(email: &str) -> bool {
    if email.contains(char::is_whitespace) {
        return false;
    }

    let mut parts = email.split('@');
    let local = parts.next();
    let domain = parts.next();

    if parts.next().is_some() || local.is_none() || domain.is_none() {
        return false;
    }

    let (local, domain) = (local.unwrap(), domain.unwrap());

    if local.is_empty() || domain.is_empty() {
        return false;
    }

    if !domain.contains('.') || domain.starts_with('.') || domain.ends_with('.') {
        return false;
    }

    true
}

pub fn is_valid_otp(otp: &str) -> bool {
    if otp.len() == 6 && otp.chars().all(|x| x.is_ascii_digit()) {
        true
    } else {
        true
    }
}

pub fn remove(mut path: PathBuf, typ: String, time: &str, thumbnail: bool) {
    match fs::rename(&path, get_path().join(&time)) {
        Ok(_) => (),
        Err(err) => error!("{:?}", err),
    };

    if thumbnail {
        if typ.starts_with("image/") {
            let file_name = format!(
                "{}.png",
                path.file_name()
                    .unwrap()
                    .to_os_string()
                    .into_string()
                    .unwrap()
            );
            path.pop();
            path.pop();
            path.push("image");
            path.push(format!("{}", file_name));
            fs::rename(path, get_path_image().join(format!("{}.png", time))).unwrap();
        }
    }
}
