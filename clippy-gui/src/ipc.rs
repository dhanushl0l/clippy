#[cfg(target_family = "unix")]
pub mod ipc {
    use std::{io::Write, os::fd::FromRawFd, sync::Mutex};

    use clippy::MessageIPC;

    static STREAM: std::sync::OnceLock<Mutex<std::os::unix::net::UnixStream>> =
        std::sync::OnceLock::new();

    pub fn init_stream() {
        let fd = std::env::var("IPC").unwrap().parse::<i32>().unwrap();
        let stream = unsafe { std::os::unix::net::UnixStream::from_raw_fd(fd) };
        STREAM
            .set(Mutex::new(stream))
            .ok()
            .expect("STREAM already initialized");
    }

    pub fn send_process(message: MessageIPC) -> Result<(), Box<dyn std::error::Error>> {
        let stream = STREAM.get().expect("STREAM not initialized");
        let stream = stream.lock().unwrap();
        let mut cloned = stream.try_clone()?;
        cloned.write_all(&serde_json::to_vec(&message)?)?;
        Ok(())
    }
}

#[cfg(not(target_family = "unix"))]
pub mod ipc {
    use clippy::MessageIPC;
    use interprocess::os::windows::named_pipe::DuplexPipeStream;
    use std::{io::Write, sync::Mutex};

    static STREAM: std::sync::OnceLock<Mutex<String>> = std::sync::OnceLock::new();

    pub fn init_stream() {
        let path = std::env::var("IPC").expect("IPC env not set");
        STREAM.set(Mutex::new(path)).expect("STREAM already set");
    }

    pub fn send_process(message: MessageIPC) -> Result<(), Box<dyn std::error::Error>> {
        let path = STREAM.get().unwrap().lock()?;
        let mut stream = DuplexPipeStream::connect_by_path(path.as_str())?;
        stream.write(&serde_json::to_vec(&message)?)?;
        Ok(())
    }
}
