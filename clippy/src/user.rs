use crate::{
    Data, Edit, MessageChannel, ResopnseServerToClient, ToByteString, log_error,
    rewrite_pending_to_data,
};
use crate::{
    MessageType, ResopnseClientToServer, UserData, UserSettings,
    http::{SERVER_WS, get_token, get_token_serv, health},
    set_global_update_bool,
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
use std::io;
use std::{error::Error, sync::Arc, time::Duration};
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::{
    select,
    sync::mpsc::Receiver,
    time::{Instant, sleep},
};

pub fn start_cloud(rx: &mut Receiver<MessageChannel>, mut usersettings: UserSettings) {
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
            health(&client, rx, &user_data).await;
            if let Err(e) = get_token_serv(&usercred, &client).await {
                error!("unable to get secure key from server");
                debug!("{}", e);
                health(&client, rx, &user_data).await;
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
                    health(&client, rx, &user_data).await;
                    continue;
                }
            };

            if let Err(e) = check_uptodate_state(&mut ws, &user_data).await {
                error!("Unable to check client state");
                debug!("{}", e);
            };
            if let Err(e) = handle_connection(&mut ws, &user_data, &mut usersettings, rx).await {
                error!("Unable to maintain connection");
                debug!("{}", e);
            };

            health(&client, rx, &user_data).await;
        }
    });
}

async fn check_uptodate_state<T: AsyncRead + AsyncWrite + Unpin + 'static>(
    ws: &mut Framed<T, Codec>,
    user_data: &UserData,
) -> Result<(), Box<dyn Error>> {
    let state = user_data.get_sync_30();
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
                if let Err(e) = handle_mag(msg, usersettings, user_data, ws, &mut last_pong, &mut buffer, &mut current_type).await{
                    error!("Unable to process message: {}",e);
                };
            }

            Some(va) = rx.recv() => {
            match va {
                MessageChannel::New { path, time, typ } => {
                    user_data.add_pending(time, Edit::New { path: path.into(), typ, }).await;
                }
                MessageChannel::Edit { path, old_id, new_id, typ } => {
                    debug!("path {} old_id {} new_time {} typ {}",path,old_id,new_id,typ);
                    user_data.add_pending(old_id, Edit::Edit { path: path.into(), typ: typ, new_id, }).await;
                }
                MessageChannel::SettingsChanged => {
                    *usersettings = UserSettings::build_user()?;
                    debug!("change settings");
                    break Ok(());
                },
                MessageChannel::Remove(id)  => {
                    user_data.add_pending(id.clone(), Edit::Remove).await;
                }
        }
            }
            Some((last, id, edit)) = user_data.next() => {
                match edit {
                    Edit::Remove => {
                        let buffer = ResopnseClientToServer::Remove(id.clone());
                        if ws
                            .send(ws::Message::Text(buffer.to_bytestring().unwrap()))
                            .await
                            .is_err()
                        {
                            return Err("Unable to send data to server".into());
                        }
                        user_data.change_state(&id);
                    }
                    Edit::New { path, typ } => match File::open(&path).await {
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
                                    user_data.change_state(&id);
                                    last_pong = Instant::now();
                                }
                                Err(e) => {
                                    error!("Failed to read file data: {}", e);
                                    user_data.pop_pending(&id);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to open file {:?}: {}", path, e);
                            user_data.pop_pending(&id);
                        }
                    },
                    Edit::Edit { path, typ, new_id } => match File::open(&path).await {
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
                                    user_data.change_state(&id);
                                    last_pong = Instant::now();
                                }
                                Err(e) => {
                                    error!("Failed to read file data: {:?} {}", path, e);
                                    user_data.pop_pending(&id);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to open file {:?}: {}", path, e);
                            user_data.pop_pending(&id);
                        }
                    },
                }
            }

        }
    }
}

async fn process_text<T: AsyncRead + AsyncWrite + Unpin + 'static>(
    bin: Bytes,
    usersettings: &UserSettings,
    user_data: &UserData,
    ws: &mut Framed<T, Codec>,
    last_pong: &mut Instant,
) -> Result<(), io::Error> {
    let state: ResopnseServerToClient = serde_json::from_slice(&bin)?;
    match state {
        ResopnseServerToClient::Success { old, new } => {
            let old_id = old;
            let (edit, _state) = match user_data.pop_pending(&old_id) {
                Some(v) => v,
                None => {
                    debug!("{:?}", user_data);
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        "Error removing pending data",
                    ));
                }
            };

            match edit {
                Edit::New { path, typ } => {
                    rewrite_pending_to_data(
                        path,
                        typ,
                        &new.clone().unwrap(),
                        usersettings.store_image,
                    );
                    user_data.add_data(new.unwrap(), usersettings.max_clipboard);
                }
                Edit::Edit { path, typ, new_id } => {
                    debug!("old edit data id: {:?}| new item id: {}", &old_id, new_id);
                    rewrite_pending_to_data(path, typ, &new.unwrap(), usersettings.store_image);
                    user_data.add_data(new_id, usersettings.max_clipboard);
                }
                Edit::Remove => {
                    user_data.remove_and_remove_file(&old_id)?;
                }
            }
            info!("Surcess sending new data");
            set_global_update_bool(true);
        }
        ResopnseServerToClient::Outdated => {
            let state = user_data.get_30_data();
            let data = ResopnseClientToServer::CheckVersionArr(state);
            if let Err(e) = ws
                .send(ws::Message::Text(serde_json::to_string(&data)?.into()))
                .await
            {
                error!("unable to send initial state {}", e);
            };
        }
        ResopnseServerToClient::Data {
            data,
            is_it_last,
            new_id,
        } => match serde_json::from_str::<Data>(&data) {
            Ok(data) => {
                log_error!(data.just_write_paste(&new_id, is_it_last, false));
                user_data.add_data(new_id, usersettings.max_clipboard);
            }
            Err(e) => {
                error!("Unable to process the data");
                debug!("{}", e)
            }
        },
        ResopnseServerToClient::Remove(id) => {
            for id in id.iter().rev() {
                log_error!(user_data.remove_and_remove_file(&id));
            }
            set_global_update_bool(true)
        }
        ResopnseServerToClient::EditReplace {
            data,
            is_it_last,
            old_id,
            new_id,
        } => match serde_json::from_str::<Data>(&data) {
            Ok(data) => {
                log_error!(data.just_write_paste(&new_id, is_it_last, false));
                user_data.add_data(new_id, usersettings.max_clipboard);
                log_error!(user_data.remove_and_remove_file(&old_id));
            }
            Err(e) => {
                error!("Unable to process the data");
                debug!("{}", e)
            }
        },
        _ => {}
    }
    *last_pong = Instant::now();
    Ok(())
}

async fn handle_mag<T: AsyncRead + AsyncWrite + Unpin + 'static>(
    msg: Frame,
    usersettings: &UserSettings,
    user_data: &UserData,
    ws: &mut Framed<T, Codec>,
    last_pong: &mut Instant,
    buffer: &mut Option<BytesMut>,
    current_type: &mut Option<MessageType>,
) -> Result<(), String> {
    match msg {
        ws::Frame::Text(txt) => {
            if let Err(e) = process_text(txt, usersettings, user_data, ws, last_pong).await {
                error!("Error saving data!");
                debug!("{e}")
            }
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
                            if let Err(e) =
                                process_text(complete, usersettings, user_data, ws, last_pong).await
                            {
                                error!("Error saving data!");
                                debug!("{e}")
                            };
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
