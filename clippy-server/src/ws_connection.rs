use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
    time::Duration,
};

use actix_ws::{Message, MessageStream, Session};
use chrono::Utc;
use clippy::Resopnse;
use futures_util::StreamExt;
use log::{debug, error};
use tokio::{
    select,
    sync::broadcast::Sender,
    time::{self, Instant},
};

use crate::{DATABASE_PATH, MessageState, UserState, get_filename, to_zip};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn ws_connection(
    mut session: Session,
    mut msg_stream: MessageStream,
    tx: Sender<MessageState>,
    state: actix_web::web::Data<UserState>,
    user: String,
) {
    let mut last_pong = Instant::now();
    let mut heartbeat = time::interval(HEARTBEAT_INTERVAL);
    let mut rx = tx.subscribe();

    loop {
        select! {
            _ = heartbeat.tick() => {
                if let Err(e) = session.ping(b"").await {
                    eprintln!("Ping failed: {e}");
                    break;
                }

                if Instant::now().duration_since(last_pong) > CLIENT_TIMEOUT {
                    eprintln!("Client heartbeat timed out");
                    break;
                }
            }

            msg = msg_stream.next() => {
                match msg {
                    Some(Ok(msg)) => match msg {
                        Message::Pong(_) => {
                            last_pong = Instant::now();
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
                                                            if let Err(e) = session.binary(data).await {}
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
                            let mut path: PathBuf = PathBuf::new().join(format!("{}/{}/", DATABASE_PATH, user));
                            match std::fs::create_dir_all(&path){
                                Ok(_) => {},
                                Err(e) => error!("unable to create user dir {}",e)
                            };

                            let file_name = get_filename(Utc::now().timestamp(),path.clone());
                            path.push(&file_name);
                            let bin = bin.to_vec();

                            if let Some(pos) = bin.iter().position(|&b| b == b'\n') {
                                let header = &bin[..pos];
                                let file_data = &bin[pos + 1..];

                                let name = String::from_utf8_lossy(header);
                                let mut file = File::create(&path).unwrap();
                                file.write_all(file_data).unwrap();
                                debug!("Saved file: {name}");
                                tx.send(MessageState::NewPath(path));
                                let file:Resopnse = Resopnse::  Success { old: name.to_string(), new: file_name.clone() };
                                let file_str = serde_json::to_string(&file).unwrap();
                                session.text(file_str).await.unwrap();
                            } else {
                                error!("No header found");
                            }
                        },
                        Message::Continuation(vsl) => {
                            println!("{:?}",vsl)
                        },
                        Message::Nop => {
                            println!("nop")
                        },
                        Message::Ping(ping) => {
                            println!("{:?}",ping)
                        },
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
                        // match val {
                        //     MessageState::NewPath(path) => {
                        //         let data = fs::read(path).unwrap();
                        //         if let Err(e) = session.binary(data).await {}
                        //     }
                        // }
                    }
                    Err(e) => {
                        eprintln!("Broadcast receive error: {e}");
                        break;
                    }
                }
            }
        }
    }
    println!("WebSocket session closed");
}
