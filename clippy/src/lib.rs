pub mod encryption_decryption;
pub mod http;
pub mod ipc;
pub mod local;
pub mod macros;
pub mod read_clipboard;
pub mod user;
pub mod write_clipboard;

use base64::Engine;
use base64::engine::general_purpose;
use bytestring::ByteString;
use encryption_decryption::{decrypt_file, encrept_file};
use image::{ImageReader, load_from_memory};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::error::Error;
use std::fs::create_dir;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::{
    collections::BTreeSet,
    env,
    fs::File,
    fs::{self},
    io::{self},
    path::PathBuf,
};
use tokio::sync::Notify;
use tokio::sync::mpsc::Sender;

#[cfg(target_os="windows")]
use crate::write_clipboard::copy_to_clipboard;
#[cfg(target_family = "unix")]
use crate::write_clipboard::copy_to_unix;

pub const APP_ID: &str = "org.clippy.clippy";
pub const API_KEY: Option<&str> = option_env!("KEY");
const IMAGE_DATA: &[u8] = include_bytes!("../../assets/gui_icons/image.png");
#[cfg(debug_assertions)]
const GUI_BIN: &str = "target/debug/clippy-gui";
#[cfg(all(not(debug_assertions), target_family = "unix"))]
const GUI_BIN: &str = "clippy-gui";
#[cfg(all(not(debug_assertions), not(target_family = "unix")))]
const GUI_BIN: &str = "clippy-gui";

static GLOBAL_BOOL: AtomicBool = AtomicBool::new(true);

pub fn set_global_bool(value: bool) {
    GLOBAL_BOOL.store(value, Ordering::SeqCst);
}

pub fn get_global_bool() -> bool {
    GLOBAL_BOOL.load(Ordering::SeqCst)
}

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

    pub fn just_write_paste(&self, id: &str, copy: bool, paste: bool) -> Result<(), io::Error> {
        let path = get_path();
        fs::create_dir_all(&path)?;
        let file_path = &path.join(id);
        let mut file = File::create(file_path)?;
        let json_data = serde_json::to_vec(self)?;
        file.write_all(&json_data)?;
        if self.typ.starts_with("image/") {
            save_image(&id, &general_purpose::STANDARD.decode(&self.data).unwrap())?;
        }
        if copy {
            #[cfg(target_family = "unix")]
            copy_to_unix(self.clone(), paste)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            #[cfg(target_os="windows")]
            copy_to_clipboard(self.clone(), paste).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        }
        set_global_update_bool(true);
        Ok(())
    }

    pub fn write_to_json(
        &self,
        tx: &Sender<MessageChannel>,
        time: String,
    ) -> Result<(), io::Error> {
        let path = get_path_pending();
        fs::create_dir_all(&path)?;
        let file_path = &path.join(&time);

        let json_data = serde_json::to_vec(self)?;

        let mut file = File::create(file_path)?;
        file.write_all(&json_data)?;

        match tx.try_send(MessageChannel::New {
            path: file_path.to_str().unwrap().into(),
            typ: self.typ.clone(),
            time,
        }) {
            Ok(_) => (),
            Err(err) => warn!(
                "Failed to send file '{}' to channel: {}",
                file_path.display(),
                err
            ),
        }
        set_global_update_bool(true);
        Ok(())
    }

    pub fn re_write_json(
        &self,
        tx: &Sender<MessageChannel>,
        new_id: String,
        old_id: String,
        path: PathBuf,
    ) -> Result<(), io::Error> {
        log_error!(fs::remove_file(path));
        let path = get_path_image();
        let old_path = path.join(&format!("{}.png", old_id));
        if old_path.is_file() {
            let new_path = path.join(&format!("{}.png", new_id));
            fs::rename(&old_path, &new_path)?;
        }
        let path = get_path_pending();
        fs::create_dir_all(&path)?;
        let file_path = &path.join(&new_id);
        let json_data = serde_json::to_string(self)?;
        let mut file = File::create(file_path)?;
        file.write_all(json_data.as_bytes())?;
        match tx.try_send(MessageChannel::Edit {
            new_id,
            old_id,
            path: file_path.to_str().unwrap().to_string(),
            typ: self.typ.clone(),
        }) {
            Ok(_) => (),
            Err(err) => warn!(
                "Failed to send file '{}' to channel: {}",
                file_path.display(),
                err
            ),
        }
        set_global_update_bool(true);
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
        let path = get_image_path(id)?;
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

    pub fn get_meta_data(&self) -> Option<String> {
        let Some(data) = self.get_data() else {
            return Some(String::new());
        };

        let lines = data.lines().take(10).map(|line| {
            let line = line.trim();
            if line.len() > 100 {
                format!("{}..", &line.chars().take(65).collect::<String>())
            } else {
                line.trim().to_string()
            }
        });

        let display_text = lines.collect::<Vec<_>>().join("\n");

        Some(display_text)
    }

    pub fn build(path: &PathBuf) -> Result<Self, io::Error> {
        let mut buf = String::new();
        let mut file = File::open(path)?;
        file.read_to_string(&mut buf)?;
        Ok(serde_json::from_str(&buf)?)
    }
}

#[derive(PartialEq, Debug)]
pub enum DataState {
    WaitingToSend,
    SentButNotAcked,
}

#[derive(Debug, Clone)]
pub struct UserData {
    data: Arc<Mutex<BTreeSet<String>>>,
    pending: Arc<Mutex<BTreeMap<String, (Edit, DataState)>>>,
    notify: Arc<Notify>,
}

impl UserData {
    fn build() -> Self {
        let mut data = BTreeSet::new();
        let mut pending = BTreeMap::new();

        Self::build_pending(&mut pending);
        Self::build_data(&mut data);

        let notify = Notify::new();

        Self {
            data: Arc::new(Mutex::new(data)),
            pending: Arc::new(Mutex::new(pending)),
            notify: Arc::new(notify),
        }
    }

    fn build_pending(pending: &mut BTreeMap<String, (Edit, DataState)>) {
        let data_path = get_path_pending();
        if let Ok(entries) = fs::read_dir(data_path) {
            for dir in entries {
                if let Ok(entry) = dir {
                    if let Ok(metadata) = File::open(entry.path()) {
                        if let Ok(data) = serde_json::from_reader::<File, Data>(metadata) {
                            if let Some(name) = entry.file_name().to_str() {
                                pending.insert(
                                    name.to_string(),
                                    (
                                        Edit::New {
                                            path: entry.path(),
                                            typ: data.typ,
                                        },
                                        DataState::WaitingToSend,
                                    ),
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    fn build_data(data: &mut BTreeSet<String>) {
        let data_path = get_path();
        if let Ok(entries) = fs::read_dir(data_path) {
            for dir in entries {
                if let Ok(entry) = dir {
                    if let Ok(metadata) = entry.metadata() {
                        if metadata.is_file() {
                            if let Some(name) = entry.file_name().to_str() {
                                data.insert(name.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    fn get_sync_30(&self) -> Vec<String> {
        self.data
            .lock()
            .unwrap()
            .iter()
            .rev()
            .take(30)
            .cloned()
            .collect()
    }

    pub async fn next(&self) -> Option<(bool, String, Edit)> {
        loop {
            let mut found: Option<(&String, &Edit)> = None;
            let mut count = 0;
            let val = self.pending.lock().unwrap();
            for (k, v) in val.iter() {
                if v.1 == DataState::WaitingToSend {
                    count += 1;
                    if found.is_none() {
                        found = Some((k, &v.0));
                    } else {
                        break;
                    }
                }
            }

            if let Some((k, v)) = found {
                let is_last = count == 1;
                return Some((is_last, k.clone(), v.clone()));
            }

            self.notify.notified().await;
        }
    }

    async fn add_pending(&self, id: String, act: Edit) {
        self.notify.notify_one();
        self.pending
            .lock()
            .unwrap()
            .insert(id, (act, DataState::WaitingToSend));
    }

    fn change_state(&self, id: &str) {
        let mut data = self.pending.lock().unwrap();
        if let Some(val) = data.get_mut(id) {
            val.1 = DataState::SentButNotAcked
        }
    }

    fn pop_pending(&self, id: &str) -> Option<(Edit, DataState)> {
        let mut data = self.pending.lock().unwrap();
        data.remove(id)
    }

    pub fn get_30_data(&self) -> Vec<String> {
        self.data
            .lock()
            .unwrap()
            .iter()
            .rev()
            .take(30)
            .cloned()
            .collect()
    }

    pub fn add_data(&self, id: String, total: Option<u32>) {
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
                    if let Ok(file) = File::open(&path) {
                        if let Ok(clipboard) = serde_json::from_reader::<_, Data>(file) {
                            if clipboard.pined {
                                pined_path.push(&i);
                                fs::rename(&path, &pined_path).unwrap();
                                pined_path.pop();
                            } else {
                                log_error!(fs::remove_file(&path));
                            }
                        } else {
                            println!("{:?} : to do find the cause of the error", path);
                            error!("file is correpted!")
                        }
                    } else {
                        println!("{:?} : to do find the cause of the error", path);
                        error!("file is correpted!")
                    }
                    path.pop();
                    data.remove(&i);
                }
                debug!("User clipboard count: {}", data.len());
            }
        }
    }

    pub fn remove_and_remove_file(&self, id: &str) -> Result<(), std::io::Error> {
        if let Ok(mut va) = self.data.lock() {
            va.remove(id);
        }
        let mut path = get_path();
        path.push(id);
        match fs::remove_file(&path) {
            Ok(_) => Ok(()),
            Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
                info!("File is already removed");
                debug!("path of remove file {:?}", path);
                Ok(())
            }
            Err(e) => {
                return Err(e.into());
            }
        }
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
    pub always_on_top: bool,
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
            always_on_top: true,
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

    pub fn update(&mut self) -> Result<(), Box<dyn Error>> {
        let new_self = Self::build_user()?;
        *self = new_self;
        Ok(())
    }

    pub fn build_user() -> Result<Self, Box<dyn Error>> {
        let mut user_config = get_path_local();
        user_config.push("user");
        if !user_config.is_dir() {
            create_dir(&user_config)?;
        }
        user_config.push(".settings");
        let file = fs::read(&user_config)?;
        let mut settings: UserSettings = match serde_json::from_slice(&file) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to parse user settings: {e}");
                if let Err(err) = fs::remove_file(&user_config) {
                    error!("Also failed to remove corrupted settings file: {err}");
                }
                UserSettings::new()
            }
        };
        user_config.pop();
        user_config.push(".user");
        let file = if let Ok(data) = fs::read(&user_config) {
            Some(data)
        } else {
            None
        };
        let key_bytes: String = match API_KEY {
            Some(va) => va.to_string(),
            None => env::var("KEY")
                .expect("Environment variable KEY is not set")
                .to_string(),
        };

        if let Some(file) = file {
            match decrypt_file(key_bytes.as_bytes(), &file) {
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

        Ok(settings)
    }

    pub fn write_local(&self) -> Result<(), Box<dyn Error>> {
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Edit {
    New {
        path: PathBuf,
        typ: String,
    },
    // edit represent add new entry and remove the old one
    Edit {
        path: PathBuf,
        typ: String,
        new_id: String,
    },
    Remove,
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
pub enum ResopnseClientToServer {
    Updated,
    Outdated,
    CheckVersion(String),
    CheckVersionArr(Vec<String>),
    Error(String),
    Data {
        data: String,
        id: String,
        last: bool,
        is_it_edit: Option<String>,
    },
    Remove(String),
}

pub trait ToByteString: Serialize {
    fn to_bytestring(&self) -> Result<ByteString, serde_json::Error> {
        let json = serde_json::to_string(self)?;
        Ok(ByteString::from(json))
    }
}

impl ToByteString for ResopnseClientToServer {}
impl ToByteString for ResopnseServerToClient {}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ResopnseServerToClient {
    Data {
        data: String,
        is_it_last: bool,
        new_id: String,
    },
    Success {
        old: String,
        new: Option<String>,
    },
    Remove(VecDeque<String>),
    EditReplace {
        data: String,
        is_it_last: bool,
        old_id: String,
        new_id: String,
    },
    Updated,
    Outdated,
}

pub enum MessageType {
    Text,
    Binary,
}

#[derive(Serialize, Deserialize)]
pub enum MessageIPC {
    None,
    OpentGUI,
    Paste(Data, bool),
    New(Data),
    Edit(EditData),
    UpdateSettings(UserSettings),
    Delete(PathBuf, String),
    Updated,
    Close,
}

#[derive(Serialize, Deserialize)]
pub struct EditData {
    data: Data,
    id: String,
    path: PathBuf,
}

impl EditData {
    pub fn new(data: Data, id: String, path: PathBuf) -> Self {
        Self { data, id, path }
    }
}

pub enum MessageChannel {
    New {
        path: String,
        time: String,
        typ: String,
    },
    Edit {
        path: String,
        old_id: String,
        new_id: String,
        typ: String,
    },
    Remove(String),
    SettingsChanged,
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

pub fn store_image(id: &[String], target_dir: PathBuf) -> Result<(), Box<dyn Error>> {
    for i in id {
        let mut path = target_dir.clone();
        path.push(i);

        let file = fs::read_to_string(path)?;
        let data: Data = serde_json::from_str(&file)?;

        if let Some(val) = data.get_image() {
            save_image(i, &val)?;
        }
    }
    Ok(())
}

pub fn get_image_path(id: &PathBuf) -> Option<PathBuf> {
    let mut path = get_path_image();
    let file_nema = format!("{}.png", id.file_name()?.to_str()?);
    path.push(file_nema);
    Some(path)
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

#[cfg(target_os = "linux")]
pub fn copy_to_linux(data: Data, paste_on_click: bool) {
    use crate::write_clipboard::{copy_to_clipboard, copy_to_clipboard_wl};
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        log_error!(copy_to_clipboard_wl(data, paste_on_click));
    } else if std::env::var("DISPLAY").is_ok() {
        log_error!(copy_to_clipboard(data, paste_on_click));
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

pub fn rewrite_pending_to_data(path: PathBuf, typ: String, time: &str, thumbnail: bool) {
    if let Err(err) = fs::rename(&path, get_path().join(&time)) {
        error!("unable to rewrite data: {:?}", err)
    };

    if thumbnail && typ.starts_with("image/") {
        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            let image_path = get_path_image();
            let old_img_file_name = format!("{}.png", file_name);
            let old_img = image_path.join(&old_img_file_name);
            let new_image = image_path.join(format!("{}.png", time));

            if let Err(e) = fs::rename(&old_img, &new_image) {
                log::error!("Failed to rename file: {}", e);
            }
        }
    }
}

pub fn save_image(time: &str, data: &[u8]) -> Result<(), io::Error> {
    let path: PathBuf = crate::get_path_image();

    fs::create_dir_all(&path)?;

    let img_path = path.join(format!("{}.png", time));
    let mut img_file = File::create(img_path)?;

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
