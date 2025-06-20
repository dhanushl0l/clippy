#[cfg(target_os = "linux")]
use crate::write_clipboard;
use crate::{
    Pending, Resopnse, UserCred, UserData, UserSettings, extract_zip,
    http::{get_token, get_token_serv, health},
    read_data_by_id, remove, set_global_update_bool,
};
use actix_codec::Framed;
use actix_codec::{AsyncRead, AsyncWrite};
use awc::{
    http::header,
    ws::{self, Codec},
};
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use reqwest::{self, Client};
use std::{collections::HashMap, error::Error, sync::Arc, thread};
use tokio::{fs::File, io::AsyncReadExt, select, sync::mpsc::Receiver};

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
        let user_data = UserData::build();

        actix_rt::System::new().block_on(async {
            loop {
                log::debug!("starting WebSocket client");
                let client = Arc::new(Client::new());
                health(&client, &mut rx, &mut pending).await;
                if let Err(e) = get_token_serv(&usercred, &client).await {
                    error!("unable to connect to server");
                    debug!("{}", e);
                    health(&client, &mut rx, &mut pending).await;
                    continue;
                };
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
                        health(&client, &mut rx, &mut pending).await;
                        continue;
                    }
                };

                if let Err(e) = check_uptodate_state(&mut ws, &user_data).await {
                    error!("Unable to connect to server");
                    debug!("{}", e);
                };
                if let Err(e) = handle_connection(
                    &mut ws,
                    &user_data,
                    &mut surcess,
                    &usersettings,
                    &mut pending,
                    &mut rx,
                )
                .await
                {
                    error!("Unable to connect to server");
                    debug!("{}", e);
                };

                pending = Pending::build().unwrap_or_else(|e| {
                    error!("{}", e);
                    Pending::new()
                });
                health(&client, &mut rx, &mut pending).await;
            }
        })
    });
}

fn past_last(last: &str) {
    let data = read_data_by_id(last);
    match data {
        Ok(val) => {
            #[cfg(not(target_os = "linux"))]
            write_clipboard::copy_to_clipboard(val).unwrap();

            #[cfg(target_os = "linux")]
            write_clipboard::copy_to_linux(val).unwrap();
        }
        Err(err) => {
            warn!("{}", err)
        }
    }
}

async fn check_uptodate_state<T: AsyncRead + AsyncWrite + Unpin + 'static>(
    ws: &mut Framed<T, Codec>,
    user_data: &UserData,
) -> Result<(), Box<dyn Error>> {
    let state = user_data.get_30();
    let data = Resopnse::CheckVersionArr(state);
    Ok(ws
        .send(ws::Message::Text(
            serde_json::to_string(&data).unwrap().into(),
        ))
        .await?)
}

async fn handle_connection<T: AsyncRead + AsyncWrite + Unpin + 'static>(
    ws: &mut Framed<T, Codec>,
    user_data: &UserData,
    surcess: &mut HashMap<String, (String, String)>,
    usersettings: &UserSettings,
    pending: &mut Pending,
    rx: &mut Receiver<(String, String, String)>,
) -> Result<(), Box<dyn Error>> {
    loop {
        select! {
            Some(msg) = ws.next() => {
                match msg? {
                    ws::Frame::Text(txt) => {
                        let state: Resopnse = serde_json::from_slice(&txt).unwrap();
                        match state {
                            Resopnse::Success {old,new} => {
                                let (path, typ) = surcess.remove(&old).unwrap();
                                remove(path, typ, &new, usersettings.store_image);
                                user_data.add(new, usersettings.max_clipboard);
                                info!("Surcess sending new data");
                                set_global_update_bool(true);
                            },
                            Resopnse::Outdated => {
                                let state = user_data.get_30();
                                let data = Resopnse::CheckVersionArr(state);
                                if let Err(e) = ws.send(ws::Message::Text(serde_json::to_string(&data).unwrap().into())).await{
                                    error!("unable to send initial state {}",e);
                                };
                            }
                            _ => {}
                        }

                    }
                    ws::Frame::Binary(bin) => {
                        let val = extract_zip(bin).unwrap();
                        if let Some(val) = val.last() {
                            if *val > user_data.last_one() {
                                past_last(val);
                            }
                        }
                        user_data.add_vec(val);
                    }
                    ws::Frame::Ping(p) => {
                        ws.send(ws::Message::Pong(p)).await.unwrap();
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
                                match ws.send(ws::Message::Binary(Bytes::from(buffer))).await
                                {
                                    Ok(_) => {
                                        surcess.insert(id, (path, typ));
                                    }
                                    Err(e) => {
                                        debug!("unable to send data to server\n{}", e);
                                        ws.send(ws::Message::Ping(Bytes::new())).await?;
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Failed to read file data: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to open file {:?}: {}", path, e);
                    }
                }
            }
        }
        if let Err(e) = send_pending(ws, surcess, pending).await {
            error!("Unable to connect to server");
            debug!("{}", e);
        };
    }
}

async fn send_pending<T: AsyncRead + AsyncWrite + Unpin + 'static>(
    ws: &mut Framed<T, Codec>,
    surcess: &mut HashMap<String, (String, String)>,
    pending: &mut Pending,
) -> Result<(), Box<dyn Error>> {
    while let Some((path, id, typ)) = pending.get() {
        match File::open(&path).await {
            Ok(mut file) => {
                let mut file_data = Vec::new();
                match file.read_to_end(&mut file_data).await {
                    Ok(_) => {
                        let mut buffer = Vec::new();
                        buffer.extend_from_slice(format!("{}\n", id).as_bytes());
                        buffer.extend_from_slice(&file_data);
                        ws.send(ws::Message::Binary(Bytes::from(buffer))).await?;
                        surcess.insert(id, (path, typ));
                        pending.pop();
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
    Ok(())
}
