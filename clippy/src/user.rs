use crate::write_clipboard;
use crate::{
    MessageType, Pending, Resopnse, UserCred, UserData, UserSettings, extract_zip,
    http::{SERVER_WS, get_token, get_token_serv, health},
    read_data_by_id, remove, set_global_update_bool,
};
use actix_codec::Framed;
use actix_codec::{AsyncRead, AsyncWrite};
use actix_http::ws::Item;
use awc::{
    http::header,
    ws::{self, Codec, Message},
};
use bytes::{Bytes, BytesMut};
use futures_util::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use reqwest::{self, Client};
use std::{error::Error, sync::Arc, thread, time::Duration};
use tokio::{
    fs::File,
    io::AsyncReadExt,
    select,
    sync::mpsc::Receiver,
    time::{Instant, sleep},
};

pub fn start_cloud(
    mut rx: Receiver<(String, String, String)>,
    usercred: UserCred,
    usersettings: UserSettings,
) {
    thread::spawn(move || {
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
                let config_ws = awc::Client::builder()
                    .max_http_version(awc::http::Version::HTTP_11)
                    .finish();
                let result = config_ws
                    .ws(SERVER_WS)
                    .set_header(header::AUTHORIZATION, format!("Bearer {}", token))
                    .max_frame_size(20 * 1024 * 1024) // 20 MB
                    .connect()
                    .await;

                let (_res, mut ws) = match result {
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
                if let Err(e) =
                    handle_connection(&mut ws, &user_data, &usersettings, &mut pending, &mut rx)
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
    usersettings: &UserSettings,
    pending: &mut Pending,
    rx: &mut Receiver<(String, String, String)>,
) -> Result<(), Box<dyn Error>> {
    let mut buffer: Option<BytesMut> = None;
    let mut current_type: Option<MessageType> = None;
    let mut last_pong = Instant::now();
    loop {
        select! {
            _ = sleep(Duration::from_secs(1)) => {
                if last_pong.elapsed() > Duration::from_secs(15) {
                    error!("No pong in time. Disconnecting.");
                    return Err("Server is out".into());
                } else if last_pong.elapsed() > Duration::from_secs(5) {
                    let _ = ws.send(Message::Ping(Bytes::new())).await;
                }
            }

            Some(msg) = ws.next() => {
                last_pong = Instant::now();
                match msg? {
                    ws::Frame::Text(txt) => {
                        process_text(txt,pending,usersettings,user_data,ws,&mut last_pong).await;
                    }
                    ws::Frame::Binary(bin) => {
                        process_bin(bin, user_data,&mut last_pong).await;
                    }
                    ws::Frame::Ping(p) => {
                        if ws.send(ws::Message::Pong(p)).await.is_err() {
                            return Err("Unable to connect to server".into());
                        }
                    }
                    ws::Frame::Pong(_) => {
                    }

                    ws::Frame::Continuation(bin) => {
                        match bin {
                            Item::FirstText(data) => {
                                buffer = Some(BytesMut::from(&data[..]));
                                current_type = Some(MessageType::Text);
                            }

                            Item::FirstBinary(data) => {
                                buffer = Some(BytesMut::from(&data[..]));
                                current_type = Some(MessageType::Binary);
                            }

                            Item::Continue(data) => {
                                if let Some(buf) = &mut buffer {
                                    buf.extend_from_slice(&data);
                                } else {
                                    error!("Received CONTINUE without FIRST. Dropping.");
                                    buffer = None;
                                    current_type = None;
                                }
                            }

                            Item::Last(data) => {
                                if let (Some(mut buf), Some(msg_type)) = (buffer.take(), current_type.take()) {
                                    buf.extend_from_slice(&data);
                                    let complete = buf.freeze();
                                    match msg_type {
                                        MessageType::Text => process_text(complete,pending,usersettings,user_data,ws,&mut last_pong).await,
                                        MessageType::Binary => process_bin(complete, user_data,&mut last_pong).await,
                                    }
                                } else {
                                    error!("Received LAST without FIRST. Dropping.");
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            Some((last,id, value)) = pending.next() => {
                match File::open(&value.path).await {
                    Ok(mut file) => {
                        let mut file_data = Vec::new();
                        match file.read_to_end(&mut file_data).await {
                            Ok(_) => {
                                let mut buffer = Vec::new();
                                buffer.extend_from_slice(format!("{}\n{}\n", id,last).as_bytes());
                                buffer.extend_from_slice(&file_data);
                                if ws.send(ws::Message::Binary(Bytes::from(buffer))).await.is_err() {
                                    return Err("Unable to connect to server".into());
                                }
                                pending.change_state(&id);
                                last_pong = Instant::now();
                            }
                            Err(e) => {
                                error!("Failed to read file data: {}", e);
                                pending.pop(&id);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to open file {:?}: {}", value.path, e);
                        pending.pop(&id);
                    }
                }
            }
            Some((path, id, typ)) = rx.recv() => {
                pending.add(path, id, typ);
            }

        }
    }
}

async fn process_text<T: AsyncRead + AsyncWrite + Unpin + 'static>(
    bin: Bytes,
    pending: &mut Pending,
    usersettings: &UserSettings,
    user_data: &UserData,
    ws: &mut Framed<T, Codec>,
    last_pong: &mut Instant,
) {
    let state: Resopnse = serde_json::from_slice(&bin).unwrap();
    match state {
        Resopnse::Success { old, new } => {
            let value = match pending.remove(&old) {
                Some(v) => v,
                None => {
                    debug!("Error removing pending data");
                    debug!("{:?}", pending);
                    return;
                }
            };

            remove(value.path, value.typ, &new, usersettings.store_image);
            user_data.add(new, usersettings.max_clipboard);
            info!("Surcess sending new data");
            set_global_update_bool(true);
        }
        Resopnse::Outdated => {
            let state = user_data.get_30();
            let data = Resopnse::CheckVersionArr(state);
            if let Err(e) = ws
                .send(ws::Message::Text(
                    serde_json::to_string(&data).unwrap().into(),
                ))
                .await
            {
                error!("unable to send initial state {}", e);
            };
        }
        _ => {}
    }
    *last_pong = Instant::now();
}

async fn process_bin(bin: Bytes, user_data: &UserData, last_pong: &mut Instant) {
    let val = extract_zip(bin).unwrap();
    if let Some(val) = val.last() {
        if *val > user_data.last_one() {
            past_last(val);
        }
    }
    user_data.add_vec(val);
    set_global_update_bool(true);
    *last_pong = Instant::now();
}
