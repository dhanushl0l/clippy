use crate::ipc::ipc::send_process;
use clippy::{Data, EditData, UserSettings, log_error};
use clippy_gui::set_lock;
use egui::{self, *};
use log::error;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

pub fn item_card_image(
    data: &mut Data,
    ui: &mut Ui,
    texture: &egui::TextureHandle,
    pinned: &mut bool,
    settings: &UserSettings,
    changed: Arc<Mutex<bool>>,
    path: &PathBuf,
    ctx: &Context,
    sync: &bool,
) -> Response {
    let frame = Frame::group(ui.style())
        .corner_radius(9)
        .outer_margin(Margin::same(4));

    let width = ctx.screen_rect().width();

    frame
        .show(ui, |ui| {
            ui.set_max_height(100.0);

            if width >= 650.0 {
                ui.set_width(600.0);
            }

            ui.vertical(|ui| {
                if ui.add(egui::ImageButton::new(texture)).clicked() {
                    log_error!(send_process(clippy::MessageIPC::Paste(
                        data.clone(),
                        settings.paste_on_click && settings.click_on_quit
                    )));

                    if settings.click_on_quit {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }

                ui.vertical_centered(|ui| {
                    ui.separator();
                    ui.horizontal(|ui| {
                        if *sync && settings.get_sync().is_none() || !*sync {
                            let pin_response = ui.selectable_label(*pinned, "📌");
                            if pin_response.clicked() {
                                if pin_response.clicked() {
                                    data.change_pined();
                                    if let Some(file_name) =
                                        path.file_name().and_then(|f| f.to_str())
                                    {
                                        let msg = clippy::MessageIPC::Edit(EditData::new(
                                            data.clone(),
                                            file_name.to_string(),
                                            path.to_path_buf(),
                                        ));
                                        log_error!(send_process(msg));
                                    }
                                    set_lock!(changed, true);
                                }
                            }

                            let delete_response = ui.selectable_label(false, "🗑");
                            if delete_response.clicked() {
                                if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
                                    let msg = clippy::MessageIPC::Delete(
                                        path.to_path_buf(),
                                        file_name.to_string(),
                                    );
                                    log_error!(send_process(msg));
                                    set_lock!(changed, true);
                                }
                            }
                        }

                        if *sync {
                            let sync = ui.selectable_label(false, "🔄");
                            sync.on_hover_text("update in progress");
                        }
                    });
                });
            });
        })
        .response
}
