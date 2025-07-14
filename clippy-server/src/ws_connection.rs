use std::{fs::File, io::Write, path::PathBuf, time::Duration};

use actix_web::web::{self, Bytes};
use actix_ws::{AggregatedMessage, AggregatedMessageStream, Session};
use chrono::Utc;
use clippy::Resopnse;
use futures_util::StreamExt;
use log::{debug, error};
use tokio::{
    select,
    sync::broadcast::Sender,
    time::{Instant, sleep},
};

use crate::{DATABASE_PATH, RoomManager, ServResopnse, UserState, get_filename, to_zip};

pub async fn ws_connection(
    mut session: Session,
    mut msg_stream: AggregatedMessageStream,
    tx: Sender<ServResopnse>,
    state: actix_web::web::Data<UserState>,
    user: String,
    room: web::Data<RoomManager>,
    pos: usize,
) {
    let mut last_pong = Instant::now();
    let mut rx = tx.subscribe();
    let mut old = String::new();

    loop {
        select! {
            msg = msg_stream.next() => {
                match msg {
                    Some(Ok(msg)) => match msg {
                        AggregatedMessage::Ping(ping) => {
                            let _ = session.pong(&ping).await;
                        }
                        AggregatedMessage::Pong(_) => {
                            last_pong = Instant::now();
                        }
                        AggregatedMessage::Text(txt) => {
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
                                    Resopnse::Data{data,id,last,is_it_edit} => {
                                        if let Some(val) = is_it_edit {
                                            if let Err(e) = state.remove(&user, &val){
                                                error!("unable to remove edited state");
                                                debug!("{}",e)
                                            };
                                        }
                                        handle_bin(&user,&state,&tx,&mut session,data,id,last,&mut old).await;
                                    }
                                    _ => {}
                                }
                            }
                            last_pong = Instant::now();
                        }
                        AggregatedMessage::Binary(_bin) => {
                            continue;
                        },
                        AggregatedMessage::Close(reason) => {
                            debug!("Client closed: {:?}", reason);
                            break;
                        }
                    }
                    Some(Err(e)) => {
                        error!("Stream error: {e}");
                        break;
                    }
                    None => {
                        error!("Client disconnected");
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
                        error!("Broadcast receive error: {e}");
                        break;
                    }
                }
                last_pong = Instant::now();
            }
            _ = sleep(Duration::from_secs(1)) => {
                if last_pong.elapsed() > Duration::from_secs(300) {
                    error!("No pong in time. Disconnecting.");
                    return;
                } else if last_pong.elapsed() > Duration::from_secs(5) {
                    let _ = session.ping(&Bytes::new()).await;
                }
            }
        }
    }
    room.remove(user, pos).await;
    error!("WebSocket session closed");
}

async fn handle_bin(
    user: &str,
    state: &actix_web::web::Data<UserState>,
    tx: &Sender<ServResopnse>,
    session: &mut Session,
    data: String,
    id: String,
    last: bool,
    old: &mut String,
) {
    let mut path: PathBuf = PathBuf::new().join(format!("{}/{}/", DATABASE_PATH, user));
    match std::fs::create_dir_all(&path) {
        Ok(_) => {}
        Err(e) => error!("unable to create user dir {}", e),
    };

    let file_name = get_filename(Utc::now().timestamp(), path.clone());
    path.push(&file_name);
    let mut file = File::create(&path).unwrap();
    file.write_all(data.as_bytes()).unwrap();
    state.update(&user, &file_name);
    debug!("Saved file: {id}");
    if last {
        if let Err(e) = tx.send(ServResopnse::New(file_name.clone())) {
            error!("error sending state: {}", e);
        };
        *old = file_name.clone();
    }
    let file: Resopnse = Resopnse::Success {
        old: id.to_string(),
        new: file_name.clone(),
    };
    let file_str = serde_json::to_string(&file).unwrap();
    session.text(file_str).await.unwrap();
}
