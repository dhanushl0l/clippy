use std::{fs, io::Write, path::PathBuf};

use clippy::{Data, create_past_lock, set_global_bool};
use egui::{self, *};

pub fn item_card_image(
    ui: &mut Ui,
    texture: &egui::TextureHandle,
    pinned: &mut bool,
    click_on_quit: bool,
    data: &Data,
    changed: &mut bool,
    path: &PathBuf,
    ctx: &Context,
) -> Response {
    let max_size = vec2(500.0, 100.0);

    let frame = Frame::group(ui.style())
        .corner_radius(9)
        .outer_margin(Margin::same(4));

    frame
        .show(ui, |ui| {
            ui.set_max_size(max_size);

            ui.vertical(|ui| {
                if ui.add(egui::ImageButton::new(texture)).clicked() {
                    set_global_bool(true);

                    // #[cfg(target_os = "linux")]
                    // clippy_gui::copy_to_linux(
                    //     "image/png".to_string(),
                    //     data.get_image_as_string().unwrap().to_string(),
                    // );

                    // #[cfg(not(target_os = "linux"))]
                    // write_clipboard::push_to_clipboard(
                    //     "image/png".to_string(),
                    //     data.get_image_as_string().unwrap().to_string(),
                    // )
                    // .unwrap();

                    create_past_lock(path);

                    if click_on_quit {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }

                ui.vertical_centered(|ui| {
                    ui.separator();
                    ui.horizontal(|ui| {
                        let pin_response = ui.selectable_label(*pinned, "📌");
                        if pin_response.clicked() {
                            if let Ok(val) = fs::read_to_string(&path) {
                                if let Ok(mut data) = serde_json::from_str::<Data>(&val) {
                                    data.change_pined();

                                    if let Ok(new_val) = serde_json::to_string_pretty(&data) {
                                        let _ = fs::File::create(&path).and_then(|mut file| {
                                            file.write_all(new_val.as_bytes())
                                        });
                                    }
                                }
                            }
                            *changed = true;
                        }

                        let delete_response = ui.selectable_label(false, "🗑");
                        if delete_response.clicked() {
                            fs::remove_file(path);
                            *changed = true;
                        }
                    });
                });
            });
        })
        .response
}
