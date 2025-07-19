use log::error;
use tokio::sync::mpsc::Receiver;

use crate::{MessageChannel, Pending, UserData, UserSettings};

pub fn start_local(rx: &mut Receiver<MessageChannel>, mut usersettings: UserSettings) {
    let pending = Pending::build().unwrap_or_else(|e| {
        error!("{}", e);
        Pending::new()
    });
    let user_data = UserData::build();

    for i in pending.data {
        user_data.add(i.0, usersettings.max_clipboard);
    }

    actix_rt::System::new().block_on(async {
        while let Some(msg) = rx.recv().await {
            match msg {
                MessageChannel::Edit {
                    path: _,
                    old_id: _,
                    time,
                    typ: _,
                } => {
                    user_data.add(time, usersettings.max_clipboard);
                }
                MessageChannel::New {
                    path: _,
                    time,
                    typ: _,
                } => {
                    user_data.add(time, usersettings.max_clipboard);
                }
                MessageChannel::SettingsChanged => {
                    usersettings = UserSettings::build_user().unwrap();
                    break;
                }
            }
        }
    });
}
