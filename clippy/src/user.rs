use crate::{
    Pending, UserCred, UserData, UserSettings,
    http::{self, download, get_token_serv, health, state},
};
use log::{debug, error, info, warn};
use reqwest::{self, Client};
use std::{sync::Arc, thread, time};
use tokio::{join, runtime::Runtime, sync::mpsc::Receiver, time::sleep};

pub fn start_cloud(
    mut rx: Receiver<(String, String)>,
    usercred: UserCred,
    usersettings: UserSettings,
) {
    thread::spawn(move || {
        debug!("Start thread 3");
        let user_data = Arc::new(UserData::build());
        let pending = Arc::new(Pending::new());
        let client = Arc::new(Client::new());
        let usercred = Arc::new(usercred);

        let async_runtime = Runtime::new().unwrap();

        async_runtime.block_on(async {
            match get_token_serv(&usercred, &client).await {
                Ok(_) => debug!("Fetched a new authentication token on start"),
                Err(err) => {
                    warn!("Unable to fetch authentication token on start");
                    debug!("{}", err);
                }
            };

            let task1 = tokio::spawn({
                let pending = pending.clone();
                let client = client.clone();
                let usercred = usercred.clone();
                let user_data = user_data.clone();

                async move {
                    while let Some((path, id)) = rx.recv().await {
                        debug!("New clipboard data: path = {}, id = {}", path, id);
                        user_data.add(id.clone(), usersettings.max_clipboard);
                        match http::send(&path, &id, &usercred, &client).await {
                            Ok(_) => (),
                            Err(err) => {
                                pending.add((id, path));
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
                async move {
                    loop {
                        let mut api_health = false;
                        if let Some((id, path)) = pending.get() {
                            match http::send(&path, &id, &usercred, &client).await {
                                Ok(_) => {
                                    pending.remove();
                                    info!("Surcess sending pending data");
                                }
                                Err(err) => {
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
                                        Err(err) => warn!("Downloade updated files error: {}", err),
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

            let results = join!(task1, task2, task3);
            for (i, result) in [results.0, results.1, results.2].into_iter().enumerate() {
                if let Err(e) = result {
                    error!("Task {} panicked: {}", i + 1, e);
                    std::process::exit(1);
                }
            }
        });
    });
}
