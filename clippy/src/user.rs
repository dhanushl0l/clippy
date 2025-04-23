use crate::{
    Pending, UserCred, UserData, UserSettings,
    encryption_decryption::{decrypt_file, encrept_file},
    get_path,
    http::{self, download, health, login, state},
};
use log::{debug, info, warn};
use reqwest::{self, Client};
use std::{
    error::Error,
    fs::{self, create_dir},
    sync::Arc,
    thread, time,
};
use tokio::{
    runtime::Runtime,
    sync::mpsc::{Receiver, Sender},
};

pub fn start_cloud(mut rx: Receiver<(String, String)>, usercred: UserCred) {
    let user_data = UserData::build();
    let user_data1 = user_data.clone();
    let pending = Pending::new();
    let pending1 = pending.clone();
    let usercred1 = usercred.clone();

    login(&usercred);

    // thread::spawn(move || {
    //     debug!("Start thread 2");
    //     loop {
    //         match state(&user_data, &client, &usercred1) {
    //             Ok(val) => {
    //                 if val {
    //                     thread::sleep(time::Duration::from_secs(5));
    //                     info!("Database updated");
    //                 } else {
    //                     match download(&user_data, &client) {
    //                         Ok(_) => debug!("Downloade updated files"),
    //                         Err(err) => warn!("{}", err),
    //                     }
    //                 }
    //             }
    //             Err(err) => health(&client),
    //         }
    //     }
    // });

    thread::spawn(move || {
        debug!("Start thread 3");
        let async_runtime = Runtime::new().unwrap();

        async_runtime.block_on(async {
            let client = Client::new();

            while let Some((path, id)) = rx.recv().await {
                match http::send(&path, &id, &user_data1, &usercred, &client).await {
                    Ok(_) => (),
                    Err(err) => {
                        pending1.add((id, path));
                        warn!("Failed to send recent clipboard: {}", err);
                    }
                };
            }
        });
    });
}
