#[cfg(target_family = "unix")]
pub mod ipc {
    use crate::write_clipboard::copy_to_unix;
    use crate::{GUI_BIN, MessageChannel, MessageIPC};
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
        use crate::{MessageIPC, get_path_local};

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

        let process = Command::new(GUI_BIN)
            .env("IPC", "0")
            .stdin(unsafe { Stdio::from_raw_fd(child) })
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn();

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
                    MessageIPC::Updated => {}
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
                    _ => {}
                }
            }
        }

        Ok(process?.kill()?)
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
pub mod ipc {}
