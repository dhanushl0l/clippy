use std::{fs, io::Write, path::PathBuf};

use clippy::Data;
use egui::{self};

use crate::Clipboard;

impl Clipboard {
    pub fn edit_window(&mut self, ctx: &egui::Context) {
        if !self.show_data_popup.0 {
            return;
        }

        let center = ctx.screen_rect().center();

        egui::Window::new("Settings")
            .default_pos(center - egui::vec2(170.0, 190.0))
            .default_height(350.0)
            .collapsible(false)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        ui.text_edit_multiline(&mut self.show_data_popup.1);
                    });

                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        let path = &self.show_data_popup.2;
                        if let Ok(val) = fs::read_to_string(path) {
                            if let Ok(mut data) = serde_json::from_str::<Data>(&val) {
                                data.change_data(&self.show_data_popup.1);

                                if let Ok(new_val) = serde_json::to_string_pretty(&data) {
                                    let _ = fs::File::create(path)
                                        .and_then(|mut file| file.write_all(new_val.as_bytes()));
                                }
                            }
                        }
                        self.changed = true;
                        self.show_data_popup = (false, String::new(), PathBuf::new());
                    }

                    if ui.button("Close").clicked() {
                        self.show_data_popup = (false, String::new(), PathBuf::new());
                        self.changed = true;
                    }
                });
            });
    }
}
