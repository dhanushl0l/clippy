use std::{
    fs::File,
    io::{Read, Write},
    path::PathBuf,
    time::Duration,
};

use actix_web::web::{self, Bytes};
use actix_ws::{AggregatedMessage, AggregatedMessageStream, Session};
use chrono::Utc;
use clippy::{ResopnseClientToServer, ResopnseServerToClient, ToByteString};
use futures_util::StreamExt;
use log::{debug, error};
use tokio::{
    select,
    sync::broadcast::Sender,
    time::{Instant, sleep},
};

use crate::{DATABASE_PATH, RoomManager, ServResopnse, UserState, get_filename};

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
                            if let Ok(parsed) = serde_json::from_str::<ResopnseClientToServer>(&txt) {
                                match parsed {
                                    ResopnseClientToServer::CheckVersion(version) =>{
                                        if state.is_updated(&user, &version){
                                            let status = ResopnseServerToClient::Updated;
                                            if let Err(e) =  session.text(serde_json::to_string(&status).unwrap()).await{
                                                debug!("Unable to send response {}",e);
                                            };
                                        }else {
                                            let status = ResopnseServerToClient::Outdated;
                                            if let Err(e) =  session.text(serde_json::to_string(&status).unwrap()).await{
                                                debug!("Unable to send response {}",e);
                                            };
                                        }
                                    }
                                    ResopnseClientToServer::CheckVersionArr(version)  =>{
                                         match state.get(&user, &version) {
                                            Some(data)=> {
                                                if data.is_empty() {
                                                    let status = ResopnseServerToClient::Updated;
                                                    if let Err(e) =  session.text(serde_json::to_string(&status).unwrap()).await{
                                                        debug!("Unable to send response {}",e);
                                                    };
                                                } else {
                                                    if let Err(e) = send_to_client(data, &mut session).await {
                                                        debug!("Unable to send response {}",e);
                                                        break;
                                                    };
                                                }
                                            },
                                            None =>  {
                                                let status = ResopnseServerToClient::Updated;
                                                if let Err(e) =  session.text(serde_json::to_string(&status).unwrap()).await{
                                                    debug!("Unable to send response {}",e);
                                                };
                                            },
                                        };
                                    }
                                    ResopnseClientToServer::Data{data,id,last,is_it_edit} => {
                                        handle_bin(&user,&state,&tx,&mut session,data,id,last,&mut old,is_it_edit).await;
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
                                    let status = ResopnseServerToClient::Outdated;
                                    if let Err(e) =  session.text(serde_json::to_string(&status).unwrap()).await {
                                        debug!("Unable to send response {}",e);
                                    };
                                }
                            },
                            ServResopnse::Remove(remove) => {
                                if remove != old {
                                    let status = ResopnseServerToClient::Edit(remove);
                                    if let Err(e) =  session.text(serde_json::to_string(&status).unwrap()).await {
                                        debug!("Unable to send response {}",e);
                                    };
                                    let status = ResopnseServerToClient::Outdated;
                                    if let Err(e) =  session.text(serde_json::to_string(&status).unwrap()).await {
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
    is_it_edit: Option<String>,
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
    if let Some(val) = is_it_edit {
        *old = file_name.clone();
        state
            .add_edit(&user, crate::Edit::Remove(val.clone()))
            .unwrap();
        if let Err(e) = tx.send(ServResopnse::Remove(val)) {
            error!("error sending state: {}", e);
        };
    } else if last {
        *old = file_name.clone();
        if let Err(e) = tx.send(ServResopnse::New(file_name.clone())) {
            error!("error sending state: {}", e);
        };
    }
    let file: ResopnseServerToClient = ResopnseServerToClient::Success {
        old: id.to_string(),
        new: file_name.clone(),
    };
    let file_str = serde_json::to_string(&file).unwrap();
    session.text(file_str).await.unwrap();
}

async fn send_to_client(
    data: Vec<(String, String)>,
    session: &mut Session,
) -> Result<(), actix_ws::Closed> {
    for (i, (path, new_id)) in data.iter().enumerate() {
        match File::open(&path) {
            Ok(mut va) => {
                let mut buf = String::new();
                if let Err(e) = va.read_to_string(&mut buf) {
                    error!("{}", e);
                    continue;
                };
                session
                    .text(
                        ResopnseServerToClient::Data {
                            data: buf,
                            is_it_last: (i == data.len() - 1),
                            new_id: new_id.to_string(),
                        }
                        .to_bytestring()
                        .unwrap(),
                    )
                    .await?;
            }
            Err(e) => {
                error!("{}", e)
            }
        }
    }
    Ok(())
}
