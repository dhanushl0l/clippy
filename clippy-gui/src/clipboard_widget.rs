use std::{fs, io::Write, path::PathBuf};

use clippy::{Data, set_global_bool};
use clippy_gui::Thumbnail;
use egui::{self, *};

pub fn item_card(
    ui: &mut Ui,
    text: &str,
    text_label: &Thumbnail,
    pinned: &mut bool,
    deleted: &mut bool,
    click_on_quit: bool,
    show_data_popup: &mut (bool, String, PathBuf),
    changed: &mut bool,
    path: &PathBuf,
    ctx: &Context,
) -> Response {
    let max_size = vec2(500.0, 100.0);

    let background_color = ui.style().visuals.window_fill;

    let frame = Frame::group(ui.style())
        .corner_radius(9)
        .outer_margin(Margin::same(4));

    frame
        .show(ui, |ui| {
            ui.set_max_size(max_size);

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

                    #[cfg(not(target_os = "linux"))]
                    write_clipboard::push_to_clipboard("String".to_string(), text.to_string())
                        .unwrap();

                    #[cfg(target_os = "linux")]
                    clippy_gui::copy_to_linux(
                        "text/plain;charset=utf-8".to_string(),
                        text.to_string(),
                    );

                    set_global_bool(false);

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
                            fs::remove_file(path);
                            *changed = true;
                        }

                        let view_all = ui.selectable_label(false, "💬");

                        if view_all.clicked() {
                            *show_data_popup = (true, text.to_string(), path.clone());
                        }
                    });
                });
            });
        })
        .response
}
