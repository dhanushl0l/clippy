use std::{
    collections::VecDeque,
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

use crate::{DATABASE_PATH, MessageMPC, RoomManager, UserState, get_filename};

pub async fn ws_connection(
    mut session: Session,
    mut msg_stream: AggregatedMessageStream,
    tx: Sender<MessageMPC>,
    state: actix_web::web::Data<UserState>,
    user: String,
    room: web::Data<RoomManager>,
) {
    let mut last_pong = Instant::now();
    let mut rx = tx.subscribe();
    let mut old = true;

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
                                                let data = ResopnseServerToClient::Remove(state.get_remove(&user));
                                                if let Err(e) = session.text(serde_json::to_string(&data).unwrap()).await{
                                                    debug!("Unable to send response {}",e);
                                                };
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
                                        handle_bin(&user, &state, &tx, &mut session, data, id, last, &mut old, is_it_edit).await;
                                    },
                                    ResopnseClientToServer::Remove(id) => {
                                        state.remove_and_add_edit(&user, &id).unwrap();
                                        old = false;
                                        tx.send(MessageMPC::Remove(id)).unwrap();
                                    },
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
                        if old {
                            match val {
                                MessageMPC::Remove(id) => {
                                    let mut vec = VecDeque::new();
                                    vec.push_front(id);
                                    let status = ResopnseServerToClient::Remove(vec);
                                    if let Err(e) =  session.text(serde_json::to_string(&status).unwrap()).await {
                                        debug!("Unable to send response {}",e);
                                    };
                                },
                                MessageMPC::New(id) =>{
                                    let path = format!("{}/{}/{}", DATABASE_PATH, user, id);
                                    if let Ok(mut file) = File::open(path){
                                        let mut buf = String::new();
                                            if let Err(e) = file.read_to_string(&mut buf) {
                                                error!("{}", e);
                                                continue;
                                            };
                                        let status = ResopnseServerToClient::Data { data: buf, is_it_last: true, new_id: id };
                                        if let Err(e) =  session.text(serde_json::to_string(&status).unwrap()).await {
                                            debug!("Unable to send response {}",e);
                                        };
                                    }
                                },
                                MessageMPC::Edit{old_id, new_id} => {
                                    let path = format!("{}/{}/{}", DATABASE_PATH, user, new_id);
                                    if let Ok(mut file) = File::open(path){
                                        let mut buf = String::new();
                                            if let Err(e) = file.read_to_string(&mut buf) {
                                                error!("{}", e);
                                                continue;
                                            };
                                        let status = ResopnseServerToClient::EditReplace { data: buf, is_it_last: true, old_id, new_id };
                                        if let Err(e) =  session.text(serde_json::to_string(&status).unwrap()).await {
                                            debug!("Unable to send response {}",e);
                                        };
                                    }
                                }
                                MessageMPC::None => {}
                            }

                        }
                        old = true;
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
    error!("WebSocket session closed");
}

async fn handle_bin(
    user: &str,
    state: &actix_web::web::Data<UserState>,
    tx: &Sender<MessageMPC>,
    session: &mut Session,
    data: String,
    id: String,
    last: bool,
    old: &mut bool,
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
    if let Some(edit) = is_it_edit {
        *old = false;
        let message = MessageMPC::Edit {
            old_id: id.clone(),
            new_id: file_name.clone(),
        };
        state.remove_and_add_edit(&user, &id).unwrap();
        debug!("edit => old if: {}| new id: {}", id, file_name);
        if let Err(e) = tx.send(message) {
            error!("error sending state: {}", e);
        };
    } else if last {
        *old = false;
        if let Err(e) = tx.send(MessageMPC::New(file_name.clone())) {
            error!("error sending state: {}", e);
        };
    }
    let file: ResopnseServerToClient = ResopnseServerToClient::Success {
        old: id.to_string(),
        new: Some(file_name),
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
