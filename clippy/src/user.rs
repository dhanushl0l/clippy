use crate::{
    Data, Edit, MessageChannel, ResopnseServerToClient, ToByteString, get_image_path, get_path,
    log_error,
};
use crate::{
    MessageType, Pending, ResopnseClientToServer, UserData, UserSettings,
    http::{SERVER_WS, get_token, get_token_serv, health},
    remove, set_global_update_bool,
};
use actix_codec::Framed;
use actix_codec::{AsyncRead, AsyncWrite};
use actix_http::ws::Item;
use awc::ws::Frame;
use awc::{
    http::header,
    ws::{self, Codec, Message},
};
use bytes::{Bytes, BytesMut};
use futures_util::{SinkExt, StreamExt};
use log::{debug, error, info};
use reqwest::{self, Client};
use std::{error::Error, sync::Arc, time::Duration};
use tokio::fs;
use tokio::{
    fs::File,
    io::AsyncReadExt,
    select,
    sync::mpsc::Receiver,
    time::{Instant, sleep},
};

pub fn start_cloud(rx: &mut Receiver<MessageChannel>, mut usersettings: UserSettings) {
    let mut pending = Pending::build().unwrap_or_else(|e| {
        error!("{}", e);
        Pending::new()
    });
    let user_data = UserData::build();
    let client = Arc::new(Client::new());

    actix_rt::System::new().block_on(async {
        loop {
            let Some(usercred) = usersettings.get_sync() else {
                break;
            };
            if usersettings.disable_sync {
                break;
            }

            log::debug!("starting WebSocket client");
            health(&client, rx, &mut pending).await;
            if let Err(e) = get_token_serv(&usercred, &client).await {
                error!("unable to get secure key from server");
                debug!("{}", e);
                health(&client, rx, &mut pending).await;
                continue;
            };
            let token = get_token();
            let config_ws = awc::Client::builder()
                .max_http_version(awc::http::Version::HTTP_11)
                .finish();
            let result = config_ws
                .ws(SERVER_WS)
                .set_header(header::AUTHORIZATION, format!("Bearer {}", token))
                .max_frame_size(30 * 1024 * 1024)
                .connect()
                .await;

            let (_res, mut ws) = match result {
                Ok((resp, conn)) => (resp, conn),
                Err(e) => {
                    error!("Client connect error: {e:?}");
                    health(&client, rx, &mut pending).await;
                    continue;
                }
            };

            if let Err(e) = check_uptodate_state(&mut ws, &user_data).await {
                error!("Unable to check client state");
                debug!("{}", e);
            };
            if let Err(e) =
                handle_connection(&mut ws, &user_data, &mut usersettings, &mut pending, rx).await
            {
                error!("Unable to maintain connection");
                debug!("{}", e);
            };

            pending = Pending::build().unwrap_or_else(|e| {
                error!("{}", e);
                Pending::new()
            });
            health(&client, rx, &mut pending).await;
        }
    });
}

// fn past_last(last: &str, paste_on_click: bool) {
//     let data = read_data_by_id(last);
//     match data {
//         Ok(val) => {
//             #[cfg(not(target_os = "linux"))]
//             write_clipboard::copy_to_clipboard(val).unwrap();

//             #[cfg(target_os = "linux")]
//             write_clipboard::copy_to_unix(val, paste_on_click).unwrap();
//         }
//         Err(err) => {
//             warn!("{}", err)
//         }
//     }
// }

async fn check_uptodate_state<T: AsyncRead + AsyncWrite + Unpin + 'static>(
    ws: &mut Framed<T, Codec>,
    user_data: &UserData,
) -> Result<(), Box<dyn Error>> {
    let state = user_data.get_30();
    let data = ResopnseClientToServer::CheckVersionArr(state);
    Ok(ws
        .send(ws::Message::Text(
            serde_json::to_string(&data).unwrap().into(),
        ))
        .await?)
}

async fn handle_connection<T: AsyncRead + AsyncWrite + Unpin + 'static>(
    ws: &mut Framed<T, Codec>,
    user_data: &UserData,
    usersettings: &mut UserSettings,
    pending: &mut Pending,
    rx: &mut Receiver<MessageChannel>,
) -> Result<(), Box<dyn Error>> {
    let mut buffer: Option<BytesMut> = None;
    let mut current_type: Option<MessageType> = None;
    let mut last_pong = Instant::now();
    loop {
        select! {
                _ = sleep(Duration::from_secs(1)) => {
                    if last_pong.elapsed() > Duration::from_secs(30) {
                        error!("No pong in time. Disconnecting.");
                        return Err("Server is out".into());
                    } else if last_pong.elapsed() > Duration::from_secs(5) {
                        let _ = ws.send(Message::Ping(Bytes::new())).await;
                    }
                }

                Some(msg) = ws.next() => {
                    last_pong = Instant::now();
                    let msg = match msg {
                        Ok(va) => va,
                        Err(e) => {
                            // to do
                            debug!("{}",e);
                            continue;
                        }
                    };
                    if let Err(e) = handle_mag(msg, pending, usersettings, user_data, ws, &mut last_pong, &mut buffer, &mut current_type).await{
                        error!("{}",e);
                    };
                }

                Some((last,id, value)) = pending.next() => {
        match value {
            Edit::New { path, typ: _ } => match File::open(&path).await {
                Ok(mut file) => {
                    let mut file_data = String::new();
                    match file.read_to_string(&mut file_data).await {
                        Ok(_) => {
                            let buffer = ResopnseClientToServer::Data {
                                data: file_data,
                                id: id.clone(),
                                last,
                                is_it_edit: None,
                            };
                            if ws
                                .send(ws::Message::Text(buffer.to_bytestring().unwrap()))
                                .await
                                .is_err()
                            {
                                return Err("Unable to send data to server".into());
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
                    error!("Failed to open file {:?}: {}", value, e);
                    pending.pop(&id);
                }
            },
            Edit::Edit {
                path,
                typ: _,
                new_id,
            } => match File::open(&path).await {
                Ok(mut file) => {
                    let mut file_data = String::new();
                    match file.read_to_string(&mut file_data).await {
                        Ok(_) => {
                            let buffer = ResopnseClientToServer::Data {
                                data: file_data,
                                id: id.clone(),
                                last,
                                is_it_edit: Some(new_id.clone()),
                            };
                            if ws
                                .send(ws::Message::Text(buffer.to_bytestring().unwrap()))
                                .await
                                .is_err()
                            {
                                return Err("Unable to send data to server".into());
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
                    error!("Failed to open file {:?}: {}", value, e);
                    pending.pop(&id);
                }
            },
            Edit::Remove { id } => {}
        }
                }

                Some(va) = rx.recv() => {
                match va {
                    MessageChannel::New { path, time, typ } => {
                        pending.add(time, Edit::New { path: path.into(), typ, }, crate::DataState::WaitingToSend);
                    }
                    MessageChannel::Edit { path, old_id, time, typ } => {
                        debug!("path {} old_id {} new_time {} typ {}",path,old_id,time,typ);
                        pending.add(old_id, Edit::Edit { path: path.into(), typ: typ, new_id: time }, crate::DataState::WaitingToSend);
                    }
                    MessageChannel::SettingsChanged => {
                        *usersettings = UserSettings::build_user()?;
                        debug!("change settings");
                        break Ok(());
                    },
                    MessageChannel::Remove(id)  => {
                        pending.add(id.clone(), Edit::Remove { id }, crate::DataState::WaitingToSend);
                    }
            }
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
    let state: ResopnseServerToClient = serde_json::from_slice(&bin).unwrap();
    match state {
        ResopnseServerToClient::Success { old, new } => {
            let (edit, state) = match pending.remove(&old) {
                Some(v) => v,
                None => {
                    debug!("Error removing pending data");
                    debug!("{:?}", pending);
                    return;
                }
            };

            match edit {
                Edit::New { path, typ } => {
                    remove(path, typ, &new.clone().unwrap(), usersettings.store_image);
                    user_data.add(new.unwrap(), usersettings.max_clipboard);
                }
                Edit::Edit { path, typ, new_id } => {
                    remove(path, typ, &new.clone().unwrap(), usersettings.store_image);
                    user_data.add(new.unwrap(), usersettings.max_clipboard);
                }
                Edit::Remove { id } => {}
            }
            info!("Surcess sending new data");
            set_global_update_bool(true);
        }
        ResopnseServerToClient::Outdated => {
            let state = user_data.get_30();
            let data = ResopnseClientToServer::CheckVersionArr(state);
            if let Err(e) = ws
                .send(ws::Message::Text(
                    serde_json::to_string(&data).unwrap().into(),
                ))
                .await
            {
                error!("unable to send initial state {}", e);
            };
        }
        ResopnseServerToClient::Data {
            data,
            is_it_last,
            new_id,
        } => {
            let data: Data = serde_json::from_str(&data).unwrap();
            log_error!(data.just_write_paste(&new_id, is_it_last, false));
            user_data.add(new_id, usersettings.max_clipboard);
        }
        _ => {}
    }
    *last_pong = Instant::now();
}

async fn handle_mag<T: AsyncRead + AsyncWrite + Unpin + 'static>(
    msg: Frame,
    pending: &mut Pending,
    usersettings: &UserSettings,
    user_data: &UserData,
    ws: &mut Framed<T, Codec>,
    last_pong: &mut Instant,
    buffer: &mut Option<BytesMut>,
    current_type: &mut Option<MessageType>,
) -> Result<(), String> {
    match msg {
        ws::Frame::Text(txt) => {
            process_text(txt, pending, usersettings, user_data, ws, last_pong).await;
        }
        ws::Frame::Binary(_bin) => {}
        ws::Frame::Ping(p) => {
            if ws.send(ws::Message::Pong(p)).await.is_err() {
                return Err("Unable to send pong to server".into());
            }
        }
        ws::Frame::Pong(_) => {}

        ws::Frame::Continuation(bin) => match bin {
            Item::FirstText(data) => {
                *buffer = Some(BytesMut::from(&data[..]));
                *current_type = Some(MessageType::Text);
            }

            Item::FirstBinary(data) => {
                *buffer = Some(BytesMut::from(&data[..]));
                *current_type = Some(MessageType::Binary);
            }

            Item::Continue(data) => {
                if let Some(buf) = buffer {
                    buf.extend_from_slice(&data);
                } else {
                    error!("Received CONTINUE without FIRST. Dropping.");
                    *buffer = None;
                    *current_type = None;
                }
            }

            Item::Last(data) => {
                if let (Some(mut buf), Some(msg_type)) = (buffer.take(), current_type.take()) {
                    buf.extend_from_slice(&data);
                    let complete = buf.freeze();
                    match msg_type {
                        MessageType::Text => {
                            process_text(complete, pending, usersettings, user_data, ws, last_pong)
                                .await
                        }
                        MessageType::Binary => {}
                    }
                } else {
                    error!("Received LAST without FIRST. Dropping.");
                }
            }
        },
        _ => {}
    }
    Ok(())
}
