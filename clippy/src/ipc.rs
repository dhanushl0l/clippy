#[cfg(target_family = "unix")]
pub mod ipc {
    use crate::write_clipboard::copy_to_unix;
    use crate::{
        API_KEY, GUI_BIN, MessageChannel, MessageIPC, get_image_path, get_path_local, log_error,
    };
    use log::{debug, error, warn};
    use serde_json::Deserializer;
    use std::fs::File;
    use std::io::{BufReader, Error, Read};
    use std::os::fd::{FromRawFd, IntoRawFd};
    use std::process::{Command, Stdio};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::{env, fs, io::Write, process};
    use std::{
        io,
        os::unix::net::{UnixListener, UnixStream},
    };
    use tokio::sync::mpsc::Sender;

    pub fn startup() -> Result<UnixListener, std::io::Error> {
        let mut path = get_path_local();
        path.push(".LOCK");
        if let Err(e) = File::create(&path) {
            debug!("{}", e);
        }
        match UnixStream::connect(&path) {
            Ok(mut stream) => {
                if env::var("CLIPPY_SERVICE").is_ok() {
                    eprintln!("Another Clippy service is already running. Please stop it first.");
                    process::exit(1);
                } else {
                    let msg = serde_json::to_vec(&MessageIPC::OpentGUI)?;
                    stream.write_all(&msg)?;
                    process::exit(0);
                }
            }
            Err(_) => {
                fs::remove_file(&path)?;
            }
        }
        UnixListener::bind(&path)
    }

    fn start_gui(tx: &Sender<MessageChannel>) -> Result<(), io::Error> {
        let (parent, child) = UnixStream::pair().unwrap();
        let child = child.into_raw_fd();

        let mut process = Command::new(GUI_BIN)
            .env("IPC", "0")
            .env("KEY", API_KEY.unwrap())
            .stdin(unsafe { Stdio::from_raw_fd(child) })
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?;

        let reader = BufReader::new(parent);
        let stream = Deserializer::from_reader(reader).into_iter::<MessageIPC>();

        for msg in stream {
            if let Ok(val) = msg {
                match val {
                    MessageIPC::Paste(data, paste_on_click) => {
                        if let Err(e) = copy_to_unix(data, paste_on_click) {
                            error!("Unable to write clipboard");
                            debug!("{}", e);
                        };
                    }
                    MessageIPC::Updated => {
                        if let Err(e) = tx.try_send(MessageChannel::SettingsChanged) {
                            error!("Unable to send modification");
                            debug!("{}", e);
                        };
                    }
                    MessageIPC::New(data) => {
                        let new_id = chrono::Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string();
                        data.write_to_json(tx, new_id).unwrap();
                    }
                    MessageIPC::UpdateSettings(settings) => {
                        settings.write_local().unwrap();
                        if let Err(e) = tx.try_send(MessageChannel::SettingsChanged) {
                            warn!("Unable to store Settings");
                            debug!("{}", e);
                        };
                    }
                    MessageIPC::Edit(data) => {
                        let new_id = chrono::Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string();
                        let old_id = data.id;
                        let path = data.path;
                        data.data.re_write_json(tx, new_id, old_id, path).unwrap();
                    }
                    MessageIPC::Delete(path, id) => {
                        let img_path = get_image_path(&path);
                        if let Some(path) = img_path {
                            log_error!(fs::remove_file(path));
                        }
                        log_error!(fs::remove_file(path));
                        log_error!(tx.try_send(MessageChannel::Remove(id)));
                    }
                    MessageIPC::Close => {
                        break;
                    }
                    _ => {}
                }
            } else {
                break;
            }
        }

        Ok(process.kill()?)
    }

    pub fn ipc_check(channel: UnixListener, rx: &Sender<MessageChannel>) -> Result<(), Error> {
        let channel = channel;
        let is_it_new = Arc::new(Mutex::new(None));
        for i in channel.incoming() {
            if let Ok(mut val) = i {
                let mut buf = String::new();
                val.read_to_string(&mut buf)?;
                match serde_json::from_str(&buf)? {
                    MessageIPC::OpentGUI => {
                        let rx = rx.clone();
                        if let Ok(mut guard) = is_it_new.lock() {
                            if guard.is_none() {
                                let is_it_new_clone = Arc::clone(&is_it_new);
                                let rx_clone = rx.clone();

                                let handle = thread::spawn(move || {
                                    if let Err(e) = start_gui(&rx_clone) {
                                        error!("Error opening clippy-gui: {}", e);
                                    }

                                    if let Ok(mut inner) = is_it_new_clone.lock() {
                                        *inner = None;
                                    }
                                });
                                *guard = Some(handle);
                            }
                        }
                    }
                    _ => {}
                }
            } else {
                return Err(io::Error::other("Broken message"));
            }
        }
        Err(io::Error::other("channel ended"))
    }
}

#[cfg(target_family = "windows")]
pub mod ipc {
    use interprocess::os::windows::named_pipe::{
        DuplexPipeStream, PipeListener, PipeListenerOptions, pipe_mode,
    };
    use log::{debug, error, warn};
    use rand::{Rng, distr::Alphanumeric};
    use std::{
        env, fs,
        io::{BufReader, Error, Read, Write},
        process::{self, Stdio},
        thread,
    };
    use tokio::sync::mpsc::Sender;

    use crate::{
        GUI_BIN, MessageChannel, MessageIPC, get_image_path, log_error,
        write_clipboard::copy_to_clipboard,
    };
    use std::{io, process::Command};
    type PipelistenerTyp = PipeListener<
        interprocess::os::windows::named_pipe::pipe_mode::Bytes,
        interprocess::os::windows::named_pipe::pipe_mode::Bytes,
    >;

    pub fn startup() -> Result<PipelistenerTyp, std::io::Error> {
        let path = r"\\.\pipe\clippy";
        match DuplexPipeStream::<pipe_mode::Bytes>::connect_by_path(path) {
            Ok(conn) => {
                let mut stream = BufReader::new(conn);
                if env::var("CLIPPY_SERVICE").is_ok() {
                    error!(
                        "Another Clippy service is already running. Please stop it before starting a new one."
                    );
                    process::exit(1)
                } else {
                    stream
                        .get_mut()
                        .write_all(&serde_json::to_vec(&MessageIPC::OpentGUI)?)?;
                    process::exit(0)
                }
            }
            Err(e) => {
                let listener = PipeListenerOptions::new()
                    .path(path)
                    .create_duplex::<pipe_mode::Bytes>()?;
                debug!("{e}");
                return Ok(listener);
            }
        }
    }

    fn start_gui(tx: &Sender<MessageChannel>) -> Result<(), io::Error> {
        let random_str: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();
        let path = format!(r"\\.\pipe\{}", random_str);

        let listener: PipelistenerTyp = PipeListenerOptions::new()
            .path(path.clone())
            .create_duplex::<pipe_mode::Bytes>()?;

        let mut process = Command::new(GUI_BIN)
            .env("IPC", path)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        for i in listener.incoming() {
            if let Ok(va) = i {
                let mut reader = BufReader::new(va);
                let mut buf = String::new();
                reader.read_to_string(&mut buf)?;
                let msg = serde_json::from_str(&buf);
                if let Ok(val) = msg {
                    match val {
                        MessageIPC::Paste(data, paste_on_click) => {
                            if let Err(e) = copy_to_clipboard(data, paste_on_click) {
                                error!("Unable to write clipboard");
                                debug!("{}", e);
                            };
                        }
                        MessageIPC::Updated => {
                            if let Err(e) = tx.try_send(MessageChannel::SettingsChanged) {
                                error!("Unable to send modification");
                                debug!("{}", e);
                            };
                        }
                        MessageIPC::New(data) => {
                            let time = chrono::Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string();
                            data.write_to_json(tx, time).unwrap();
                        }
                        MessageIPC::UpdateSettings(settings) => {
                            settings.write_local().unwrap();
                            if let Err(e) = tx.try_send(MessageChannel::SettingsChanged) {
                                error!("Unable to store Settings");
                                debug!("{}", e);
                            };
                        }
                        MessageIPC::Edit(data) => {
                            let time = chrono::Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string();
                            let id = data.id;
                            let path = data.path;
                            data.data.re_write_json(tx, time, id, path).unwrap();
                        }
                        MessageIPC::Delete(path, id) => {
                            let img_path = get_image_path(&path);
                            if let Some(path) = img_path {
                                log_error!(fs::remove_file(path));
                            }
                            log_error!(fs::remove_file(path));
                            log_error!(tx.try_send(MessageChannel::Remove(id)));
                        }
                        MessageIPC::Close => {
                            break;
                        }
                        _ => {}
                    }
                } else {
                    break;
                }
            }else {
                break;
            }
        }
        Ok(process.kill()?)
    }

    pub fn ipc_check(channel: PipelistenerTyp, rx: &Sender<MessageChannel>) -> Result<(), Error> {
        loop {
            for conn in channel.incoming() {
                if let Ok(val) = conn {
                    let mut reader = BufReader::new(val);
                    let mut buf = String::new();
                    if let Err(e) = reader.read_to_string(&mut buf) {
                        error!("Problem reading pipe data try updating the app");
                        debug!("{e}");
                    };
                    let rx = rx.clone();
                    match serde_json::from_str::<MessageIPC>(&buf) {
                        Ok(MessageIPC::OpentGUI) => {
                            thread::spawn(move || {
                                if let Err(e) = start_gui(&rx) {
                                    error!("Unable to start up gui app: {}", e);
                                };
                            });
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
