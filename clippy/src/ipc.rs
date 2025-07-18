#[cfg(target_family = "unix")]
pub mod ipc {
    use crate::write_clipboard::copy_to_unix;
    use crate::{GUI_BIN, MessageChannel, MessageIPC, get_path_local};
    use log::{debug, error};
    use serde_json::Deserializer;
    use std::io::{BufReader, Read};
    use std::os::fd::{FromRawFd, IntoRawFd};
    use std::process::{Command, Stdio};
    use std::{env, fs, io::Write, process};
    use std::{
        io,
        os::unix::net::{UnixListener, UnixStream},
    };
    use tokio::sync::mpsc::Sender;

    pub fn startup() -> Result<UnixListener, std::io::Error> {
        let mut path = get_path_local();
        path.push(".LOCK");
        match UnixStream::connect(&path) {
            Ok(mut stream) => {
                if env::var("CLIPPY_SERVICE").is_ok() {
                    error!(
                        "Another Clippy service is already running. Please stop it before starting a new one."
                    );
                    process::exit(1)
                } else {
                    stream.write(&serde_json::to_vec(&MessageIPC::OpentGUI)?)?;
                    process::exit(0)
                }
            }
            Err(_) => {
                fs::remove_file(&path)?;
                let listener = UnixListener::bind(path)?;
                return Ok(listener);
            }
        };
    }

    fn start_gui(tx: &Sender<MessageChannel>) -> Result<(), io::Error> {
        let (parent, child) = UnixStream::pair().unwrap();
        let child = child.into_raw_fd();

        let mut process = Command::new(GUI_BIN)
            .env("IPC", "0")
            .stdin(unsafe { Stdio::from_raw_fd(child) })
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?;

        let reader = BufReader::new(parent);
        let stream = Deserializer::from_reader(reader).into_iter::<MessageIPC>();

        for msg in stream {
            if let Ok(val) = msg {
                match val {
                    MessageIPC::Paste(data) => {
                        if let Err(e) = copy_to_unix(data) {
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
                    }
                    MessageIPC::Edit(data) => {
                        let time = chrono::Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string();
                        let id = data.id;
                        data.data.re_write_json(tx, time, id).unwrap();
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

    pub fn ipc_check(channel: UnixListener, rx: &Sender<MessageChannel>) {
        let channel = channel;
        for i in channel.incoming() {
            if let Ok(mut val) = i {
                let mut buf = String::new();
                val.read_to_string(&mut buf).unwrap();
                match serde_json::from_str(&buf).unwrap() {
                    MessageIPC::OpentGUI => {
                        if let Err(e) = start_gui(rx) {
                            error!("{}", e);
                        };
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(not(target_family = "unix"))]
pub mod ipc {
    use interprocess::os::windows::named_pipe::{
        DuplexPipeStream, PipeListener, PipeListenerOptions, pipe_mode,
    };
    use log::{debug, error};
    use rand::{Rng, distr::Alphanumeric};
    use std::{
        env,
        io::{BufReader, Read, Write},
        process::{self, Stdio},
    };
    use tokio::sync::mpsc::Sender;

    use crate::{GUI_BIN, MessageChannel, MessageIPC, write_clipboard::copy_to_clipboard};
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
        let path = format!(r"\\.\pipe\clip-{}", random_str);

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
                        MessageIPC::Paste(data) => {
                            if let Err(e) = copy_to_clipboard(data) {
                                error!("Unable to write clipboard");
                                debug!("{}", e);
                            };
                        }
                        MessageIPC::Updated => {
                            if let Err(e) = tx.try_send(MessageChannel::SettingsChanged) {
                                println!("{e}");
                            };
                        }
                        MessageIPC::New(data) => {
                            println!("new");
                            let time = chrono::Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string();
                            data.write_to_json(tx, time).unwrap();
                        }
                        MessageIPC::UpdateSettings(settings) => {
                            settings.write_local().unwrap();
                        }
                        MessageIPC::Edit(data) => {
                            let time = chrono::Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string();
                            let id = data.id;
                            data.data.re_write_json(tx, time, id).unwrap();
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
        }
        Ok(process.kill()?)
    }

    pub fn ipc_check(channel: PipelistenerTyp, rx: &Sender<MessageChannel>) {
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
                        if let Err(e) = start_gui(&rx) {
                            error!("{}", e);
                        };
                    }
                    _ => {}
                }
            }
        }
    }
}
