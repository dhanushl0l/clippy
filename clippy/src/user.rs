use crate::{
    Pending, UserCred, UserData, UserSettings,
    http::{self, download, health, state},
    remove, set_global_update_bool,
};
use log::{debug, error, info, warn};
use reqwest::{self, Client};
use std::{
    sync::Arc,
    thread,
    time::{self},
};
use tokio::{runtime::Runtime, select, sync::mpsc::Receiver, time::sleep};

pub fn start_cloud(
    mut rx: Receiver<(String, String, String)>,
    usercred: UserCred,
    usersettings: UserSettings,
) {
    thread::spawn(move || {
        debug!("Start thread 3");
        let user_data = Arc::new(UserData::build());
        let client = Arc::new(Client::new());
        let usercred = Arc::new(usercred);
        let pending = Arc::new(Pending::build().unwrap_or_else(|e| {
            error!("{}", e);
            Pending::new()
        }));

        let async_runtime = Runtime::new().unwrap();

        async_runtime.block_on(async {
            let task1 = tokio::spawn({
                let pending = pending.clone();
                let client = client.clone();
                let usercred = usercred.clone();
                let user_data = user_data.clone();

                async move {
                    while let Some((path, id, typ)) = rx.recv().await {
                        debug!("New clipboard data: path = {}, id = {}", path, id);
                        match http::send_to_cloud(&path, &usercred, &client, &user_data, true).await
                        {
                            Ok(data) => {
                                remove(path, typ, &data, usersettings.store_image);
                                user_data.add(data, usersettings.max_clipboard);
                                info!("Surcess sending new data");
                                set_global_update_bool(true);
                            }
                            Err(err) => {
                                pending.add(path, typ).unwrap();
                                warn!("Failed to send recent clipboard: {}", err);
                            }
                        };
                    }
                }
            });

            let task2 = tokio::spawn({
                let pending = pending.clone();
                let client = client.clone();
                let usercred = usercred.clone();
                let userdata = user_data.clone();
                async move {
                    loop {
                        let mut api_health = false;
                        if let Some((path, typ)) = pending.get() {
                            let mut last = false;
                            if pending.len() == 1 {
                                last = true
                            }

                            match http::send_to_cloud(&path, &usercred, &client, &userdata, last)
                                .await
                            {
                                Ok(data) => {
                                    remove(path, typ, &data, usersettings.store_image);
                                    userdata.add(data, usersettings.max_clipboard);
                                    info!("Surcess sending pending data");
                                    set_global_update_bool(true);
                                    pending.pop();
                                }
                                Err(err) => {
                                    if err.downcast_ref::<std::io::Error>().map_or(false, |ioe| {
                                        ioe.kind() == std::io::ErrorKind::NotFound
                                    }) {
                                        error!("The clipboard data not found");
                                        pending.pop();
                                    }
                                    warn!("Failed to send pending clipboard: {}", err);
                                    api_health = true;
                                }
                            };
                        } else {
                            sleep(time::Duration::from_secs(5)).await;
                        };
                        if api_health {
                            health(&client).await;
                        }
                    }
                }
            });

            let task3 = tokio::spawn({
                let user_data = user_data.clone();
                let client = client.clone();
                let usercred = usercred.clone();
                async move {
                    let mut log = true;

                    loop {
                        let mut api_health = false;

                        match state(&user_data, &client, &usercred).await {
                            Ok(val) => {
                                if val {
                                    sleep(time::Duration::from_secs(5)).await;
                                    if log {
                                        info!("Database uptodate");
                                        log = false;
                                    }
                                } else {
                                    match download(&user_data, &client).await {
                                        Ok(_) => debug!("Downloade updated files"),
                                        Err(err) => {
                                            warn!("Downloade updated files error: {}", err)
                                        }
                                    };
                                    log = true;
                                    sleep(time::Duration::from_secs(5)).await;
                                }
                            }
                            Err(err) => {
                                log = false;
                                api_health = true;
                                warn!("unable to reach the server: {}", err);
                            }
                        }
                        if api_health {
                            health(&client).await;
                        }
                    }
                }
            });

            let result = select! {
                res = task1 => ("task1", res),
                res = task2 => ("task2", res),
                res = task3 => ("task3", res),
            };

            match result {
                (name, Ok(val)) => {
                    error!("{} failed: {:?}", name, val);
                    std::process::exit(0);
                }
                (name, Err(e)) => {
                    error!("{} failed: {}", name, e);
                    std::process::exit(1);
                }
            }
        });
    });
}
