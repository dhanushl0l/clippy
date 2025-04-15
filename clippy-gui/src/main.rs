use clippy::{Data, UserSettings, get_path, set_global_bool, write_clipboard};
use eframe::{
    App, NativeOptions,
    egui::{CentralPanel, ScrollArea, ViewportBuilder},
    run_native,
};
use egui::{
    Align, Button, Layout, Margin, RichText, Stroke, TextEdit, TextStyle, TopBottomPanel, Vec2,
};
use std::{
    cmp::Reverse,
    fs::{self},
};

struct Clipboard {
    data: Vec<(Option<(Vec<u8>, (u32, u32))>, Data)>,
    loaded: bool,
    settings: UserSettings,
    show_settings: bool,
    show_signin_window: bool,
    username: String,
}

impl Clipboard {
    fn new() -> Self {
        let mut data = Vec::new();
        if let Ok(entries) = fs::read_dir(get_path()) {
            let mut entries: Vec<_> = entries.flatten().collect();
            entries.sort_unstable_by_key(|entry| Reverse(entry.path()));
            for entry in entries {
                if entry.path().is_file() {
                    if let Ok(content) = fs::read_to_string(entry.path()) {
                        match serde_json::from_str::<Data>(&content) {
                            Ok(file) => data.push((file.get_image_thumbnail(&entry), file)),
                            Err(e) => {
                                eprintln!("Failed to parse {}: {}", entry.path().display(), e)
                            }
                        }
                    }
                }
            }
        }

        Self {
            data,
            loaded: true,
            settings: UserSettings::new(),
            show_settings: false,
            show_signin_window: false,
            username: "enter the username".to_string(),
        }
    }

    fn loaded(&mut self, state: bool) {
        self.loaded = state;
    }
}
impl App for Clipboard {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        TopBottomPanel::top("footer").show(ctx, |ui| {
            let available_width = ui.available_width();

            ui.allocate_ui(Vec2::new(available_width, 0.0), |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(20.0);

                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        ui.add_space((available_width / 2.0) - 60.0);
                        ui.label(RichText::new("Clippy").size(40.0));
                    });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(10.0);

                        let button = Button::new(RichText::new("⚙").size(20.0))
                            .min_size(Vec2::new(30.0, 30.0))
                            .corner_radius(50.0)
                            .stroke(Stroke::new(1.0, ui.visuals().widgets.inactive.bg_fill));

                        if ui.add(button).on_hover_text("Settings").clicked() {
                            self.show_settings = true;
                        }
                    });
                });
            });

            if self.show_settings {
                let mut open = true;
                egui::Window::new("Settings")
                    .open(&mut open)
                    .resizable(false)
                    .collapsible(false)
                    .fixed_pos(ctx.screen_rect().center() - egui::vec2(150.0, 100.0))
                    .show(ctx, |ui| {
                        egui::Frame::group(ui.style())
                            .inner_margin(Margin::same(20))
                            .show(ui, |ui| {
                                ui.vertical_centered(|ui| {
                                    if let Some(user_data) = self.settings.get_sync() {
                                        ui.vertical(|ui| {
                                            ui.horizontal(|ui| {
                                                ui.label("username:");
                                                ui.label(&user_data.username);
                                            });

                                            ui.horizontal(|ui| {
                                                ui.label("key");
                                                ui.label("***********");
                                            });

                                            ui.button("Log out");
                                        });
                                    } else if self.show_signin_window {
                                        ui.vertical_centered(|ui| {
                                            ui.label(
                                                RichText::new("Enter your details")
                                                    .size(20.0)
                                                    .strong(),
                                            );

                                            ui.add_space(8.0);
                                            ui.label(
                                                RichText::new("Username:").size(15.0).strong(),
                                            );

                                            ui.style_mut().override_text_style =
                                                Some(TextStyle::Heading);

                                            ui.add(
                                                TextEdit::singleline(&mut self.username)
                                                    .desired_width(300.0)
                                                    .min_size(Vec2::new(200.0, 25.0))
                                                    .vertical_align(Align::Center),
                                            );

                                            ui.style_mut().override_text_style = None;

                                            ui.add_space(10.0);

                                            ui.horizontal(|ui| {
                                                let button_size = Vec2::new(100.0, 35.0);
                                                let button_space = 30.0;
                                                ui.add_space(button_space);

                                                if ui
                                                    .add(
                                                        egui::Button::new(
                                                            RichText::new("Sign in")
                                                                .size(16.0)
                                                                .strong(),
                                                        )
                                                        .min_size(button_size),
                                                    )
                                                    .clicked()
                                                {
                                                    self.show_signin_window = false;
                                                }

                                                ui.add_space(20.0);

                                                if ui
                                                    .add(
                                                        egui::Button::new(
                                                            RichText::new("Cancel")
                                                                .size(16.0)
                                                                .strong(),
                                                        )
                                                        .min_size(button_size),
                                                    )
                                                    .clicked()
                                                {
                                                    self.show_signin_window = false;
                                                }
                                                ui.add_space(button_space);
                                            });
                                        });
                                    } else {
                                        ui.label(RichText::new("😃").size(150.0).strong());
                                        let signin_button = ui.add(
                                            Button::new(
                                                RichText::new("Enabel Sync").size(24.0).strong(),
                                            )
                                            .min_size(Vec2::new(100.0, 40.0)),
                                        );

                                        if signin_button.clicked() {
                                            self.show_signin_window = true;
                                        }
                                    }
                                });
                            });
                        ui.vertical(|ui| {
                            ui.checkbox(&mut self.settings.store_image, "Settings");
                            ui.checkbox(&mut self.settings.click_on_quit, "Change");
                        });

                        ui.separator();

                        if ui.button("Close").clicked() {
                            self.show_settings = false;
                        }
                    });

                if !open {
                    self.show_settings = false;
                }
            }
        });

        CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    for (image, i) in &self.data {
                        if let Some(dat) = i.get_data() {
                            if ui.button(&dat).clicked() {
                                set_global_bool(true);
                                #[cfg(not(target_os = "linux"))]
                                write_clipboard::push_to_clipboard("String".to_string(), dat)
                                    .unwrap();

                                #[cfg(target_os = "linux")]
                                clippy_gui::copy_to_linux(
                                    "text/plain;charset=utf-8".to_string(),
                                    dat,
                                );

                                set_global_bool(false);

                                if self.settings.click_on_quit {
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                            }
                        } else if let Some((image_data, (width, height))) = image {
                            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                                [*width as usize, *height as usize],
                                &image_data,
                            );

                            let texture = ctx.load_texture(
                                "thumbnail",
                                color_image,
                                egui::TextureOptions::LINEAR,
                            );

                            if ui.add(egui::ImageButton::new(&texture)).clicked() {
                                set_global_bool(true);

                                #[cfg(target_os = "linux")]
                                clippy_gui::copy_to_linux(
                                    "image/png".to_string(),
                                    i.get_image_as_string().unwrap().to_string(),
                                );

                                #[cfg(not(target_os = "linux"))]
                                write_clipboard::push_to_clipboard(
                                    "image/png".to_string(),
                                    i.get_image_as_string().unwrap().to_string(),
                                )
                                .unwrap();

                                set_global_bool(false);

                                if self.settings.click_on_quit {
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                            }
                        }
                    }
                });
            });
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    let ui = Clipboard::new();
    let options = NativeOptions {
        viewport: ViewportBuilder::default().with_inner_size(Vec2::new(800.0, 600.0)),
        ..Default::default()
    };
    run_native("clippy", options, Box::new(|_cc| Ok(Box::new(ui))))
}
