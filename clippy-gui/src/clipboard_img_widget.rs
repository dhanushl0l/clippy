use std::{fs, io::Write, path::PathBuf};

use clippy::{Data, create_past_lock, set_global_bool};
use egui::{self, *};

pub fn item_card_image(
    ui: &mut Ui,
    texture: &egui::TextureHandle,
    pinned: &mut bool,
    click_on_quit: bool,
    changed: &mut bool,
    path: &PathBuf,
    ctx: &Context,
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
                    set_global_bool(true);

                    match create_past_lock(path) {
                        Ok(_) => (),
                        Err(err) => eprintln!("{err}"),
                    };

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
