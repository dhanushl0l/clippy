use crate::{
    Pending, UserCred, UserData,
    http::{self, download, get_token_serv, health, state},
};
use log::{debug, info, warn};
use reqwest::{self, Client};
use std::{fs, sync::Arc, thread, time};
use tokio::{join, runtime::Runtime, sync::mpsc::Receiver, time::sleep};

pub fn start_cloud(mut rx: Receiver<(String, String)>, usercred: UserCred) {
    thread::spawn(move || {
        debug!("Start thread 3");
        let user_data = Arc::new(UserData::build());
        let pending = Arc::new(Pending::new());
        let client = Arc::new(Client::new());
        let usercred = Arc::new(usercred);

        let async_runtime = Runtime::new().unwrap();

        async_runtime.block_on(async {
            get_token_serv(&usercred, &client).await;
            let task1 = tokio::spawn({
                let user_data = user_data.clone();
                let pending = pending.clone();
                let client = client.clone();
                let usercred = usercred.clone();
                async move {
                    while let Some((path, id)) = rx.recv().await {
                        match http::send(&path, &id, &user_data, &usercred, &client).await {
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
                let user_data = user_data.clone();
                let pending = pending.clone();
                let client = client.clone();
                let usercred = usercred.clone();
                async move {
                    loop {
                        let mut api_health = false;
                        if let Some((id, path)) = pending.get() {
                            match http::send(&path, &id, &user_data, &usercred, &client).await {
                                Ok(_) => {
                                    pending.remove();
                                    info!("Surcess sending pending data");
                                }
                                Err(err) => {
                                    warn!("Failed to send recent clipboard: {}", err);
                                    api_health = true;
                                }
                            };
                        } else {
                            get_token_serv(&usercred, &client).await;
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
                    loop {
                        match state(&user_data, &client, &usercred).await {
                            Ok(val) => {
                                if val {
                                    sleep(time::Duration::from_secs(5)).await;
                                    info!("Database updated");
                                } else {
                                    match download(&user_data, &client).await {
                                        Ok(_) => debug!("Downloade updated files"),
                                        Err(err) => warn!("{}", err),
                                    };
                                    sleep(time::Duration::from_secs(5)).await;
                                }
                            }
                            Err(err) => sleep(time::Duration::from_secs(5)).await,
                        }
                    }
                }
            });

            join!(task1, task2, task3);
        });
    });
}
