use crate::{
    Pending, UserCred, UserData, UserSettings,
    http::{self, download, health, send_zip, state},
    remove, set_global_update_bool,
};
use log::{debug, error, info, warn};
use reqwest::{self, Client};
use std::{
    process,
    sync::Arc,
    thread,
    time::{self, Duration},
};
use tokio::{runtime::Runtime, select, sync::mpsc::Receiver, time::sleep};

pub fn start_cloud(pending: Arc<Pending>, usercred: UserCred, usersettings: UserSettings) {
    thread::spawn(move || {
        debug!("Start thread 3");
        let user_data = Arc::new(UserData::build());
        let client = Arc::new(Client::new());
        let usercred = Arc::new(usercred);

        let async_runtime = Runtime::new().unwrap();

        async_runtime.block_on(async {
            let task1 = tokio::spawn({
                let client = client.clone();
                let usercred = usercred.clone();
                let user_data = user_data.clone();
                async move {
                    loop {
                        let mut api_health = false;

                        if pending.data.lock().unwrap().len() > 1 {
                            match pending.get_zip() {
                                Ok(val) => {
                                    match send_zip(val, &usercred, &client).await {
                                        Ok(_) => {
                                            pending.empty();
                                        }
                                        Err(_) => (),
                                    };
                                }
                                Err(err) => {
                                    // error!("{}", err);
                                    api_health = true;
                                }
                            };
                        } else if pending.data.lock().unwrap().len() == 1 {
                            if let Some((path, typ)) = pending.get() {
                                match http::send(&path, &usercred, &client).await {
                                    Ok(data) => {
                                        remove(path, typ, &data, usersettings.store_image);
                                        user_data.add(data, usersettings.max_clipboard);
                                        info!("Surcess sending new data");
                                        set_global_update_bool(true);
                                        pending.pop();
                                    }
                                    Err(err) => {
                                        pending.add(path, typ);
                                        warn!("Failed to send recent clipboard: {}", err);
                                    }
                                };
                            }
                        } else {
                            sleep(time::Duration::from_secs(5)).await;
                        };
                        if api_health {
                            health(&client).await;
                        }
                    }
                }
            });

            let task2 = tokio::spawn({
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
                res = task1 => ("task2", res),
                res = task2 => ("task3", res),
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
