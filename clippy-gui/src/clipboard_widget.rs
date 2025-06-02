use std::{fs, io::Write, path::PathBuf};

use clippy::{Data, create_past_lock, log_eprintln, set_global_bool};
use clippy_gui::Thumbnail;
use egui::{self, *};

pub fn item_card(
    ui: &mut Ui,
    text: &str,
    text_label: &Thumbnail,
    pinned: &mut bool,
    click_on_quit: bool,
    show_data_popup: &mut (bool, String, PathBuf),
    changed: &mut bool,
    path: &PathBuf,
    ctx: &Context,
    sync: &bool,
) -> Response {
    let background_color = ui.style().visuals.window_fill;
    let width = ctx.screen_rect().width();

    let frame = Frame::group(ui.style())
        .corner_radius(9)
        .outer_margin(Margin::same(4));

    frame
        .show(ui, |ui| {
            ui.set_max_height(100.0);

            if width >= 650.0 {
                ui.set_width(600.0);
            }

            ui.vertical_centered(|ui| {
                if ui
                    .add_sized(
                        ui.available_size(),
                        egui::Button::new(if let Thumbnail::Text(val) = text_label {
                            val
                        } else {
                            text
                        })
                        .fill(background_color),
                    )
                    .clicked()
                {
                    set_global_bool(true);

                    match create_past_lock(path) {
                        Ok(_) => (),
                        Err(err) => eprintln!("{err}"),
                    };

                    if click_on_quit {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                };

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
                            log_eprintln!(fs::remove_file(path));
                            *changed = true;
                        }

                        let view_all = ui.selectable_label(false, "💬");

                        if view_all.clicked() {
                            *show_data_popup = (true, text.to_string(), path.clone());
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
