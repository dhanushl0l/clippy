use std::{fs::File, io::Write, path::PathBuf, time::Duration};

use actix_web::web::{Bytes, BytesMut};
use actix_ws::{Item, Message, MessageStream, Session};
use chrono::Utc;
use clippy::Resopnse;
use futures_util::StreamExt;
use log::{debug, error};
use tokio::{
    select,
    sync::broadcast::Sender,
    time::{self, Instant, sleep},
};

use crate::{DATABASE_PATH, ServResopnse, UserState, get_filename, to_zip};

pub async fn ws_connection(
    mut session: Session,
    mut msg_stream: MessageStream,
    tx: Sender<ServResopnse>,
    state: actix_web::web::Data<UserState>,
    user: String,
) {
    let mut last_pong = Instant::now();
    let mut rx = tx.subscribe();
    let mut buffer: Option<BytesMut> = None;
    let mut old = String::new();

    loop {
        select! {
            msg = msg_stream.next() => {
                last_pong = Instant::now();
                match msg {
                    Some(Ok(msg)) => match msg {
                        Message::Ping(ping) => {
                            let _ = session.pong(&ping).await;
                        }
                        Message::Pong(_) => {
                        }

                        Message::Text(txt) => {
                            if let Ok(parsed) = serde_json::from_str::<Resopnse>(&txt) {
                                match parsed {
                                    Resopnse::CheckVersion(version) =>{
                                        if state.is_updated(&user, &version){
                                            let status: Resopnse = Resopnse::Updated;
                                            if let Err(e) =  session.text(serde_json::to_string(&status).unwrap()).await{
                                                debug!("Unable to send response {}",e);
                                            };
                                        }else {
                                            let status: Resopnse = Resopnse::Outdated;
                                            if let Err(e) =  session.text(serde_json::to_string(&status).unwrap()).await{
                                                debug!("Unable to send response {}",e);
                                            };
                                        }
                                    }
                                    Resopnse::CheckVersionArr(version)  =>{
                                         match state.get(&user, &version) {
                                            Some(data)=> {
                                                if data.is_empty() {
                                                    let status: Resopnse = Resopnse::Updated;
                                                    if let Err(e) =  session.text(serde_json::to_string(&status).unwrap()).await{
                                                        debug!("Unable to send response {}",e);
                                                    };
                                                } else {
                                                    match to_zip(data) {
                                                        Ok(data) => {
                                                            if let Err(e) = session.binary(data).await {
                                                                error!("Error sending bin to client{}",e);
                                                            }
                                                        },
                                                        Err(err) => {
                                                            error!("{:?}", err);
                                                            let status: Resopnse = Resopnse::Error(String::from("Unable to generate tar"));
                                                            if let Err(e) =  session.text(serde_json::to_string(&status).unwrap()).await{
                                                                debug!("Unable to send response {}",e);
                                                            };
                                                        }
                                                    }
                                                }
                                            },
                                            None =>  {
                                                let status: Resopnse = Resopnse::Updated;
                                                if let Err(e) =  session.text(serde_json::to_string(&status).unwrap()).await{
                                                    debug!("Unable to send response {}",e);
                                                };
                                            },
                                        };
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Message::Close(reason) => {
                            println!("Client closed: {:?}", reason);
                            break;
                        }
                        Message::Binary(bin) => {
                            handle_bin(&user,&state,&tx,&mut session,bin,&mut old).await;
                        },
                        Message::Continuation(item) => {
                            match item {
                            Item::FirstBinary(data) => {
                                buffer = Some(BytesMut::from(&data[..]));
                            }

                            Item::Continue(data) => {
                                if let Some(buf) = &mut buffer {
                                    buf.extend_from_slice(&data);
                                } else {
                                    eprintln!("Received CONTINUE without FIRST. Dropping.");
                                    buffer = None;
                                }
                            }

                            Item::Last(data) => {
                                if let Some(mut buf) = buffer.take() {
                                    buf.extend_from_slice(&data);
                                    handle_bin(&user,&state,&tx,&mut session,buf.freeze(),&mut old).await;
                                } else {
                                    eprintln!("Received LAST without FIRST. Dropping.");
                                }
                            }
                            _ => {}
                        }
                        }
                        Message::Nop => {},
                    }
                    Some(Err(e)) => {
                        eprintln!("Stream error: {e}");
                        break;
                    }
                    None => {
                        eprintln!("Client disconnected");
                        break;
                    }
                }
            }

            result = rx.recv() => {
                match result {
                    Ok(val) => {
                        match val {
                            ServResopnse::New(new) => {
                                if new != old {
                                let status: Resopnse = Resopnse::Outdated;
                                if let Err(e) =  session.text(serde_json::to_string(&status).unwrap()).await{
                                    debug!("Unable to send response {}",e);
                                };
                            }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Broadcast receive error: {e}");
                        break;
                    }
                }
            }
            _ = sleep(Duration::from_secs(1)) => {
                if last_pong.elapsed() > Duration::from_secs(5) {
                    let _ = session.ping(&Bytes::new()).await;
                }
                if last_pong.elapsed() > Duration::from_secs(15) {
                    eprintln!("No pong in time. Disconnecting.");
                    return;
                }
            }
        }
    }
    error!("WebSocket session closed");
}

async fn handle_bin(
    user: &str,
    state: &actix_web::web::Data<UserState>,
    tx: &Sender<ServResopnse>,
    session: &mut Session,
    bin: Bytes,
    old: &mut String,
) {
    let mut path: PathBuf = PathBuf::new().join(format!("{}/{}/", DATABASE_PATH, user));
    match std::fs::create_dir_all(&path) {
        Ok(_) => {}
        Err(e) => error!("unable to create user dir {}", e),
    };

    let file_name = get_filename(Utc::now().timestamp(), path.clone());
    path.push(&file_name);
    let bin = bin.to_vec();

    if let Some(pos) = bin.iter().position(|&b| b == b'\n') {
        let header = &bin[..pos];
        let file_data = &bin[pos + 1..];

        let name = String::from_utf8_lossy(header);
        let mut file = File::create(&path).unwrap();
        file.write_all(file_data).unwrap();
        state.update(&user, &file_name);
        debug!("Saved file: {name}");
        if let Err(e) = tx.send(ServResopnse::New(file_name.clone())) {
            error!("error sending state: {}", e);
        };
        *old = file_name.clone();
        let file: Resopnse = Resopnse::Success {
            old: name.to_string(),
            new: file_name.clone(),
        };
        let file_str = serde_json::to_string(&file).unwrap();
        session.text(file_str).await.unwrap();
    } else {
        error!("No header found");
    }
}
