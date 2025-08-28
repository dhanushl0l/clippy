use log::error;
use tokio::sync::mpsc::Receiver;

use crate::{MessageChannel, UserData, UserSettings, log_error};

pub fn start_local(rx: &mut Receiver<MessageChannel>, mut usersettings: UserSettings) {
    let user_data = UserData::build();

    actix_rt::System::new().block_on(async {
        while let Some(msg) = rx.recv().await {
            match msg {
                MessageChannel::Edit {
                    path: _,
                    old_id: _,
                    new_id,
                    typ: _,
                } => {
                    user_data.add_data(new_id, usersettings.max_clipboard);
                }
                MessageChannel::New {
                    path: _,
                    time,
                    typ: _,
                } => {
                    user_data.add_data(time, usersettings.max_clipboard);
                }
                MessageChannel::SettingsChanged => {
                    usersettings = UserSettings::build_user().unwrap();
                    break;
                }
                MessageChannel::Remove(id) => {
                    log_error!(user_data.remove_and_remove_file(&id));
                }
            }
        }
    });
}
