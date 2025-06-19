use crate::{
    Pending, Resopnse, UserCred, UserData, UserSettings,
    http::{self, download, get_token, get_token_serv, health, state},
    remove, set_global_update_bool,
};
use awc::{http::header, ws};
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use reqwest::{self, Client};
use std::{
    collections::HashMap,
    io,
    sync::Arc,
    thread,
    time::{self},
};
use tokio::{
    fs::File,
    io::AsyncReadExt,
    runtime::{Builder, Runtime},
    select,
    sync::mpsc::{self, Receiver},
    time::sleep,
};

pub fn start_cloud(
    mut rx: Receiver<(String, String, String)>,
    usercred: UserCred,
    usersettings: UserSettings,
) {
    thread::spawn(move || {
        let mut surcess = HashMap::new();
        let mut pending = Pending::build().unwrap_or_else(|e| {
            error!("{}", e);
            Pending::new()
        });
        actix_rt::System::new().block_on(async {
            loop {
                let user_data = UserData::build();

                log::info!("starting echo WebSocket client");
                let client = Arc::new(Client::new());
                get_token_serv(&usercred, &client).await;
                let token = get_token();
                let result = awc::Client::new()
                    .ws("ws://0.0.0.0:7777/connect")
                    .set_header(header::AUTHORIZATION, format!("Bearer {}", token))
                    .max_frame_size(20 * 1024 * 1024) // 20 MB
                    .connect()
                    .await;

                let (res, mut ws) = match result {
                    Ok((resp, conn)) => (resp, conn),
                    Err(e) => {
                        eprintln!("Client connect error: {e:?}");
                        return;
                    }
                };

                debug!("response: {res:?}");

                'outer: loop {
                    select! {
                        Some(msg) = ws.next() => {
                            match msg {
                                Ok(ws::Frame::Text(txt)) => {
                                    let state: Resopnse = serde_json::from_slice(&txt).unwrap();
                                    match state {
                                        Resopnse::Success {old,new} => {
                                            let (path, typ) = surcess.remove(&old).unwrap();
                                            remove(path, typ, &new, usersettings.store_image);
                                            user_data.add(new, usersettings.max_clipboard);
                                            info!("Surcess sending new data");
                                            set_global_update_bool(true);
                                        },
                                        _ => {}
                                    }

                                }
                                Ok(ws::Frame::Ping(p)) => {
                                    ws.send(ws::Message::Pong(p)).await.unwrap();
                                }
                                Err(e) => {
                                    error!("WebSocket error: {}", e);
                                    break;
                                }
                                _ => {}
                            }
                        }

                        Some((path, id, typ)) = rx.recv() => {
                            match File::open(&path).await {
                                Ok(mut file) => {
                                    let mut file_data = Vec::new();
                                    match file.read_to_end(&mut file_data).await {
                                        Ok(_) => {
                                            let mut buffer = Vec::new();
                                            buffer.extend_from_slice(format!("{}\n", id).as_bytes());
                                            buffer.extend_from_slice(&file_data);
    
                                            match ws
                                                .send(ws::Message::Binary(Bytes::from(buffer)))
                                                .await
                                            {
                                                Ok(_) => {
                                                    surcess.insert(id, (path, typ));
                                                }
                                                Err(e) => {
                                                    debug!("unable to send data to server\n{}", e);
                                                    if let Err(_) =
                                                        ws.send(ws::Message::Ping(Bytes::new())).await
                                                    {
                                                        error!("Unable to connect to channel");
                                                        break 'outer;
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            error!("Failed to read file data: {}", e);                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to open file {:?}: {}", path, e);
                                    pending.pop();
                                }
                            }
                        }
                        // else => break, // optional: breaks if both streams end
                    }
                    while let Some((path, id, typ)) = pending.get() {
                            match File::open(&path).await {
                                Ok(mut file) => {
                                    let mut file_data = Vec::new();
                                    match file.read_to_end(&mut file_data).await {
                                        Ok(_) => {
                                            let mut buffer = Vec::new();
                                            buffer.extend_from_slice(format!("{}\n", id).as_bytes());
                                            buffer.extend_from_slice(&file_data);
    
                                            match ws
                                                .send(ws::Message::Binary(Bytes::from(buffer)))
                                                .await
                                            {
                                                Ok(_) => {
                                                    surcess.insert(id, (path, typ));
                                                    pending.pop();
                                                }
                                                Err(e) => {
                                                    debug!("unable to send data to server\n{}", e);
                                                    if let Err(_) =
                                                        ws.send(ws::Message::Ping(Bytes::new())).await
                                                    {
                                                        error!("Unable to connect to channel");
                                                        break 'outer;
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            error!("Failed to read file data: {}", e);
                                            pending.pop();
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to open file {:?}: {}", path, e);
                                    pending.pop();
                                }
                        }
                    }
                }
                pending = Pending::build().unwrap_or_else(|e| {
                    error!("{}", e);
                    Pending::new()
                });
                health(&client, &mut rx, &mut pending).await;
            }
        })
    });
}
