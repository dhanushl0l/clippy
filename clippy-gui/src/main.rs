use clippy::{Data, UserCred, UserSettings, get_path, set_global_bool, write_clipboard};
use eframe::{
    App, NativeOptions,
    egui::{CentralPanel, ScrollArea, ViewportBuilder},
    run_native,
};
use egui::{
    Align, Button, Label, Layout, Margin, RichText, Stroke, TextEdit, TextStyle, TopBottomPanel,
    Vec2,
};
use http::{check_user, login, signin};
use std::{
    cmp::Reverse,
    fs::{self},
};
mod http;

struct Clipboard {
    data: Vec<(Option<(Vec<u8>, (u32, u32))>, Data)>,
    loaded: bool,
    settings: UserSettings,
    show_settings: bool,
    show_signin_window: bool,
    username: String,
    key: String,
    show_login_window: bool,
    show_createuser_window: bool,
    show_error: (bool, String),
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
            settings: match UserSettings::build_user() {
                Ok(val) => val,
                Err(err) => {
                    eprintln!("{}", err);
                    UserSettings::new()
                }
            },
            show_settings: false,
            show_signin_window: false,
            show_login_window: false,
            show_createuser_window: false,
            show_error: (false, String::from("")),
            username: "enter the username".to_string(),
            key: "enter the Password".to_string(),
        }
    }
}
impl App for Clipboard {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        let button_size = Vec2::new(100.0, 35.0);

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
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                if let Some(user_data) = self.settings.get_sync() {
                                    ui.vertical_centered(|ui| {
                                        ui.label(RichText::new("😃").size(150.0).strong());

                                        ui.label(RichText::new("username:").size(12.3).strong());
                                        ui.label(
                                            RichText::new(
                                                self.settings.get_sync().clone().unwrap().username,
                                            )
                                            .size(16.0)
                                            .strong(),
                                        );

                                        ui.add_space(10.0);

                                        let button = ui.add(
                                            egui::Button::new(
                                                RichText::new("Log out").size(16.0).strong(),
                                            )
                                            .min_size(button_size),
                                        );
                                        if button.clicked() {
                                            self.settings.remove_user();
                                            self.settings.write();
                                        }

                                        ui.add_space(10.0);
                                    });
                                } else if self.show_signin_window {
                                    ui.vertical_centered(|ui| {
                                        ui.add_space(10.0);

                                        ui.label(
                                            RichText::new("Enter your details").size(20.0).strong(),
                                        );

                                        ui.add_space(8.0);
                                        ui.label(RichText::new("Username:").size(15.0).strong());

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

                                        let total_button_width =
                                            button_size.x * 2.0 + 20.0 + 2.0 * 35.0; // 2 buttons + spacing + side padding
                                        let available_width = ui.available_width();
                                        let horizontal_padding =
                                            (available_width - total_button_width).max(0.0) / 2.0;

                                        ui.horizontal(|ui| {
                                            ui.add_space(horizontal_padding);
                                            ui.add_space(35.0);

                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        egui::RichText::new("Sign in")
                                                            .size(16.0)
                                                            .strong(),
                                                    )
                                                    .min_size(button_size),
                                                )
                                                .clicked()
                                            {
                                                self.show_signin_window = false;
                                                match check_user(self.username.clone()) {
                                                    Some(false) => {
                                                        self.show_createuser_window = true;
                                                    }
                                                    Some(true) => {
                                                        self.show_login_window = true;
                                                    }
                                                    None => {
                                                        self.show_error = (
                                                            true,
                                                            "unable to connect to server"
                                                                .to_string(),
                                                        );
                                                    }
                                                }
                                            }

                                            ui.add_space(20.0);

                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        egui::RichText::new("Cancel")
                                                            .size(16.0)
                                                            .strong(),
                                                    )
                                                    .min_size(button_size),
                                                )
                                                .clicked()
                                            {
                                                self.show_signin_window = false;
                                            }

                                            ui.add_space(35.0);
                                        });
                                        ui.add_space(10.0);
                                    });
                                } else if self.show_createuser_window {
                                    let login_button = ui.button("signin");

                                    if login_button.clicked() {
                                        let user = signin(self.username.clone());
                                        if let Ok(val) = user {
                                            self.settings.set_user(val);
                                            self.show_createuser_window = false;
                                            self.settings.write();
                                        } else if let Err(err) = user {
                                            self.show_createuser_window = false;
                                            self.show_error = (true, err.to_string());
                                        }
                                    }
                                } else if self.show_login_window {
                                    ui.vertical_centered(|ui| {
                                        ui.label(RichText::new("😃").size(150.0).strong());

                                        ui.label(
                                            RichText::new("Enter your details").size(20.0).strong(),
                                        );

                                        ui.add_space(8.0);
                                        ui.label(RichText::new("Password:").size(15.0).strong());

                                        ui.style_mut().override_text_style =
                                            Some(TextStyle::Heading);

                                        ui.add(
                                            TextEdit::singleline(&mut self.key)
                                                .desired_width(300.0)
                                                .min_size(Vec2::new(200.0, 25.0))
                                                .vertical_align(Align::Center),
                                        );

                                        ui.style_mut().override_text_style = None;

                                        ui.add_space(10.0);

                                        ui.horizontal(|ui| {
                                            let total_button_width =
                                                button_size.x * 2.0 + 20.0 + 2.0 * 35.0;
                                            let available_width = ui.available_width();
                                            let horizontal_padding =
                                                (available_width - total_button_width).max(0.0)
                                                    / 2.0;

                                            ui.add_space(horizontal_padding);
                                            ui.add_space(35.0);

                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        egui::RichText::new("Login")
                                                            .size(16.0)
                                                            .strong(),
                                                    )
                                                    .min_size(button_size),
                                                )
                                                .clicked()
                                            {
                                                let user = UserCred::new(
                                                    self.username.clone(),
                                                    self.key.clone(),
                                                );
                                                match login(user.clone()) {
                                                    Ok(()) => {
                                                        self.settings.set_user(user);
                                                        self.settings.write();
                                                    }
                                                    Err(err) => {
                                                        self.show_error = (true, err.to_string())
                                                    }
                                                };
                                                self.show_login_window = false;
                                            }

                                            ui.add_space(20.0);

                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        egui::RichText::new("Cancel")
                                                            .size(16.0)
                                                            .strong(),
                                                    )
                                                    .min_size(button_size),
                                                )
                                                .clicked()
                                            {
                                                self.show_login_window = false;
                                            }

                                            ui.add_space(35.0);
                                        });
                                        ui.add_space(10.0);
                                    });
                                } else if self.show_error.0 {
                                    ui.label(&self.show_error.1);
                                    if ui.button("close").clicked() {
                                        self.show_error = (false, String::new())
                                    }
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
                                    ui.add_space(10.0);
                                }
                            });
                        });
                        ui.vertical(|ui| {
                            if ui
                                .checkbox(&mut self.settings.store_image, "Settings")
                                .clicked()
                            {
                                self.settings.write();
                            };
                            if ui
                                .checkbox(&mut self.settings.click_on_quit, "Change")
                                .clicked()
                            {
                                self.settings.write();
                            };
                        });

                        ui.separator();

                        if ui.button("Close").clicked() {
                            self.show_settings = false;
                        }
                    });

                if !open {
                    self.show_settings = false;
                    self.show_login_window = false;
                    self.show_createuser_window = false;
                    self.show_error = (false, "".to_string());
                    self.show_signin_window = false;
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
