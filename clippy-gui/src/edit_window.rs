use std::fs;

use clippy::{Data, EditData, log_error};
use clippy_gui::set_lock;
use egui::ScrollArea;
use egui::{
    self, Align, Button, CentralPanel, Color32, Layout, Margin, RichText, Stroke, TopBottomPanel,
    Vec2,
};
use log::error;

use crate::Clipboard;
use crate::ipc::ipc::send_process;

impl Clipboard {
    pub fn edit_window(&mut self, ctx: &egui::Context) {
        if !self.show_data_popup.0 {
            return;
        }

        TopBottomPanel::top("header")
            .min_height(50.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    ui.label(RichText::new("Edit view").size(40.0));

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(10.0);
                        let button = Button::new(RichText::new("✖").size(20.0))
                            .min_size(Vec2::new(30.0, 30.0))
                            .corner_radius(50.0)
                            .stroke(Stroke::new(1.0, ui.visuals().widgets.inactive.bg_fill));

                        if ui
                            .add(button)
                            .on_hover_text("Close without saving")
                            .clicked()
                        {
                            self.show_data_popup = (false, String::new(), None, false);
                            set_lock!(self.changed, true);
                        }

                        let button = Button::new(RichText::new("💾").size(20.0))
                            .min_size(Vec2::new(30.0, 30.0))
                            .corner_radius(50.0)
                            .stroke(Stroke::new(1.0, ui.visuals().widgets.inactive.bg_fill));

                        if ui.add(button).on_hover_text("Save").clicked() {
                            if let Some(path) = &self.show_data_popup.2 {
                                if let Ok(val) = fs::read_to_string(path) {
                                    if let Ok(mut data) = serde_json::from_str::<Data>(&val) {
                                        data.change_data(&self.show_data_popup.1);
                                        data.pined = self.show_data_popup.3;
                                        if let Some(file_name) =
                                            path.file_name().and_then(|f| f.to_str())
                                        {
                                            let msg = clippy::MessageIPC::Edit(EditData::new(
                                                data,
                                                file_name.to_string(),
                                                path.to_path_buf(),
                                            ));
                                            log_error!(send_process(msg));
                                        }
                                    }
                                }
                            } else {
                                log_error!(send_process(clippy::MessageIPC::New(Data::new(
                                    self.show_data_popup.1.to_string(),
                                    "text/plain;charset=utf-8".to_string(),
                                    "os".to_string(),
                                    true,
                                ))));
                            }
                            self.show_data_popup = (false, String::new(), None, false);
                        }
                        let mut button = Button::new(RichText::new("📌").size(20.0))
                            .min_size(Vec2::new(30.0, 30.0))
                            .corner_radius(50.0)
                            .stroke(Stroke::new(1.0, ui.visuals().widgets.inactive.bg_fill));

                        if self.show_data_popup.3 {
                            button = button.fill(Color32::from_hex("#1a76d2").unwrap());
                        }

                        if ui.add(button).on_hover_text("Pin").clicked() {
                            self.show_data_popup.3 = !self.show_data_popup.3;
                        }
                    });
                });
            });
        CentralPanel::default().show(ctx, |ui| {
            let mut size = ui.available_size_before_wrap();
            size.y = size.y - 101.0;
            ScrollArea::both().show(ui, |ui| {
                ui.add_sized(
                    size,
                    egui::TextEdit::multiline(&mut self.show_data_popup.1)
                        .frame(false)
                        .code_editor()
                        .margin(Margin {
                            left: 20,
                            right: 20,
                            top: 0,
                            bottom: 0,
                        }),
                );
            });
        });
    }
}
