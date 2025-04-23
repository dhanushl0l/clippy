use clipboard_img_widget::item_card_image;
use clipboard_widget::item_card;
use clippy::{
    Data, SystemTheam, UserCred, UserSettings, get_path, set_global_bool, write_clipboard,
};
use clippy_gui::{Thumbnail, Waiting, str_formate};
use custom_egui_widget::toggle;
use eframe::{
    App, NativeOptions,
    egui::{CentralPanel, ScrollArea, ViewportBuilder},
    run_native,
};
use egui::{
    Align, Align2, Button, Color32, ComboBox, Id, Label, Layout, Margin, RichText, Stroke,
    TextEdit, TextStyle, Theme, TopBottomPanel, Vec2,
};
use http::{check_user, login, signin};
use std::{
    cmp::Reverse,
    fs::{self},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};
use tokio::runtime::Runtime;
mod clipboard_img_widget;
mod clipboard_widget;
mod custom_egui_widget;
mod edit_window;
mod http;

struct Clipboard {
    data: Vec<(Thumbnail, Data, PathBuf)>,
    changed: bool,
    settings: UserSettings,
    show_settings: bool,
    show_signin_window: bool,
    username: String,
    key: String,
    waiting: Arc<Mutex<Waiting>>,
    show_login_window: bool,
    show_createuser_window: bool,
    show_error: (bool, String),
    show_data_popup: (bool, String, PathBuf),
}

impl Clipboard {
    fn new() -> Self {
        let mut data = Vec::new();
        if let Ok(entries) = fs::read_dir(get_path()) {
            let mut entries: Vec<_> = entries.flatten().collect();
            entries.sort_unstable_by_key(|entry| Reverse(entry.path()));
            for entry in entries {
                let path = entry.path();
                if path.is_file() {
                    if let Ok(content) = fs::read_to_string(&path) {
                        match serde_json::from_str::<Data>(&content) {
                            Ok(file) => {
                                if file.typ.starts_with("image/") {
                                    if let Some(val) = file.get_image_thumbnail(&entry) {
                                        data.push((Thumbnail::Image(val), file, path));
                                    }
                                } else {
                                    if let Some(val) = file.get_data() {
                                        data.push((Thumbnail::Text(str_formate(&val)), file, path));
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to parse {}: {}", path.display(), e)
                            }
                        }
                    }
                }
            }
        }

        Self {
            data,
            changed: false,
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
            username: "".to_string(),
            key: "".to_string(),
            waiting: Arc::new(Mutex::new(Waiting::None)),
            show_data_popup: (false, String::new(), PathBuf::new()),
        }
    }

    fn refresh(&mut self) {
        *self = Clipboard::new();
    }
}
impl App for Clipboard {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        let button_size = Vec2::new(100.0, 35.0);

        TopBottomPanel::top("footer").show(ctx, |ui| {
            match self.settings.theme {
                SystemTheam::System => (),
                SystemTheam::Dark => ctx.set_visuals(egui::Visuals::dark()),
                SystemTheam::Light => ctx.set_visuals(egui::Visuals::light()),
            }

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
                    .fixed_pos(ctx.screen_rect().center() - egui::vec2(170.0, 190.0))
                    .show(ctx, |ui| {
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.vertical_centered(|ui| {
                                if let Some(user_data) = self.settings.get_sync() {
                                    let user_data = user_data.clone();
                                    ui.vertical_centered(|ui| {
                                        ui.label(RichText::new("😃").size(150.0).strong());

                                        ui.label(RichText::new("username:").size(12.3).strong());
                                        ui.label(RichText::new(user_data.username).size(15.0));

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
                                        ui.label(RichText::new("Username:").size(17.0).strong());

                                        ui.add_space(8.0);

                                        ui.style_mut().override_text_style =
                                            Some(TextStyle::Heading);

                                        ui.add(
                                            TextEdit::singleline(&mut self.username)
                                                .vertical_align(Align::Center)
                                                .hint_text("enter the username"),
                                        );

                                        ui.style_mut().override_text_style = None;

                                        ui.add_space(10.0);

                                        let total_button_width =
                                            button_size.x * 2.0 + 20.0 + 2.0 * 35.0;
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
                                                let username = self.username.clone();
                                                let wait = self.waiting.clone();

                                                thread::spawn(move || {
                                                    println!("started");
                                                    let async_runtime = Runtime::new().unwrap();
                                                    let status = async_runtime.block_on(async {
                                                        check_user(username).await
                                                    });
                                                    let mut wait_lock = wait.lock().unwrap();
                                                    *wait_lock = Waiting::CheckUser(status);
                                                });
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
                                    let signin_button = ui.button("signin");

                                    if signin_button.clicked() {
                                        let wait = self.waiting.clone();
                                        let username = self.username.clone();
                                        thread::spawn(move || {
                                            let async_runtime = Runtime::new().unwrap();

                                            let signin = async_runtime
                                                .block_on(async { signin(username).await });
                                            match signin {
                                                Ok(val) => {
                                                    let mut wait_lock = wait.lock().unwrap();
                                                    *wait_lock = Waiting::Signin(Some(val));
                                                }
                                                Err(err) => {
                                                    // self.show_createuser_window = false;
                                                    // self.show_error = (true, err.to_string());
                                                }
                                            }
                                        });
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

                                        ui.add_space(8.0);

                                        ui.add(
                                            TextEdit::singleline(&mut self.key)
                                                .vertical_align(Align::Center)
                                                .hint_text("Enter the Password"),
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
                                                let wait = self.waiting.clone();
                                                thread::spawn(move || {
                                                    let async_runtime = Runtime::new().unwrap();

                                                    let login_result = async_runtime
                                                        .block_on(async { login(user).await });

                                                    match login_result {
                                                        Err(err) => {
                                                            eprintln!("error logging in {}", err)
                                                        }
                                                        Ok(val) => {
                                                            let mut wait_lock =
                                                                wait.lock().unwrap();
                                                            *wait_lock = Waiting::Login(val);
                                                        }
                                                    }
                                                });
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
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label("Theme");
                                ui.with_layout(Layout::bottom_up(Align::RIGHT), |ui| {
                                    egui::ComboBox::new("theme_selector", "")
                                        .selected_text(match self.settings.theme {
                                            SystemTheam::System => "System",
                                            SystemTheam::Light => "Light",
                                            SystemTheam::Dark => "Dark",
                                        })
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(
                                                &mut self.settings.theme,
                                                SystemTheam::System,
                                                "System",
                                            );
                                            ui.selectable_value(
                                                &mut self.settings.theme,
                                                SystemTheam::Light,
                                                "Light",
                                            );
                                            ui.selectable_value(
                                                &mut self.settings.theme,
                                                SystemTheam::Dark,
                                                "Dark",
                                            );
                                        });
                                });

                                self.settings.write();
                            });

                            ui.horizontal(|ui| {
                                ui.label("Interval");
                                ui.with_layout(Layout::bottom_up(Align::RIGHT), |ui| {
                                    if ui
                                        .add(
                                            egui::Slider::new(&mut self.settings.intrevel, 3..=30)
                                                .text(""),
                                        )
                                        .changed()
                                    {
                                        self.settings.write();
                                    }
                                });
                            });

                            ui.horizontal(|ui| {
                                ui.label("Store Image");
                                ui.with_layout(Layout::bottom_up(Align::RIGHT), |ui| {
                                    if ui.add(toggle(&mut self.settings.store_image)).changed() {
                                        self.settings.write();
                                    }
                                });
                            });

                            ui.horizontal(|ui| {
                                ui.label("Select to Quit");
                                ui.with_layout(Layout::bottom_up(Align::RIGHT), |ui| {
                                    if ui.add(toggle(&mut self.settings.click_on_quit)).changed() {
                                        self.settings.write();
                                    }
                                });
                            });

                            ui.horizontal(|ui| {
                                ui.label("Change");
                                ui.with_layout(Layout::bottom_up(Align::RIGHT), |ui| {
                                    if ui.add(toggle(&mut self.settings.click_on_quit)).changed() {
                                        self.settings.write();
                                    }
                                });
                            });

                            ui.horizontal(|ui| {
                                ui.label("Auto sync");
                                ui.with_layout(Layout::bottom_up(Align::RIGHT), |ui| {
                                    if ui.add(toggle(&mut self.settings.auto_sync)).changed() {
                                        self.settings.write();
                                    }
                                });
                            });

                            ui.horizontal(|ui| {
                                ui.label("Change");
                                ui.with_layout(Layout::bottom_up(Align::RIGHT), |ui| {
                                    if ui.add(toggle(&mut self.settings.click_on_quit)).changed() {
                                        self.settings.write();
                                    }
                                });
                            });
                        });

                        ui.separator();

                        ui.vertical_centered(|ui| {
                            ui.label("Thanks for using the app!");

                            ui.hyperlink_to(
                                "Project Repository",
                                "https://github.com/dhanushl0l/clippy",
                            );
                        });
                    });

                let wait: Arc<Mutex<Waiting>> = self.waiting.clone();
                if let Ok(mut val) = wait.try_lock() {
                    match &*val {
                        Waiting::None => (),
                        Waiting::CheckUser(Some(true)) => {
                            self.show_login_window = true;
                            *val = Waiting::None;
                        }
                        Waiting::CheckUser(Some(false)) => {
                            self.show_createuser_window = true;
                            *val = Waiting::None;
                        }
                        Waiting::Login(Some(usercred)) => {
                            self.settings.set_user(usercred.clone());
                            *val = Waiting::None;
                        }
                        Waiting::Login(None) => {
                            self.show_error = (true, String::from("Authentication failed"));
                            self.show_login_window = false;
                            *val = Waiting::None;
                        }
                        Waiting::CheckUser(None) => {
                            self.show_error = (true, String::from("Problem connectiong to server"));
                            *val = Waiting::None;
                        }
                        Waiting::Signin(None) => {
                            self.show_error = (true, String::from("Authentication failed"));
                            self.show_signin_window = false;
                            *val = Waiting::None;
                        }
                        Waiting::Signin(Some(usercred)) => {
                            self.settings.set_user(usercred.clone());
                            *val = Waiting::None;
                        }
                    }
                }

                if !open {
                    self.show_settings = false;
                    self.show_login_window = false;
                    self.show_createuser_window = false;
                    self.show_error = (false, "".to_string());
                    self.show_signin_window = false;
                }
            }

            if self.show_data_popup.0 {
                self.edit_window(ctx);
                let esc_pressed = ctx.input(|i| i.key_pressed(egui::Key::Escape));
                if esc_pressed {
                    self.show_data_popup = (false, String::new(), PathBuf::new());
                }
            }
        });

        if self.changed {
            self.refresh();
            self.changed = false;
        }

        CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    for (thumbnail, i, path) in &self.data {
                        if let Some(dat) = i.get_data() {
                            ui.add_enabled_ui(true, |ui| {
                                item_card(
                                    ui,
                                    &dat,
                                    thumbnail,
                                    &mut i.get_pined(),
                                    &mut i.get_pined(),
                                    self.settings.click_on_quit,
                                    &mut self.show_data_popup,
                                    &mut self.changed,
                                    path,
                                    ctx,
                                )
                            });
                        } else if let Thumbnail::Image((image_data, (width, height))) = thumbnail {
                            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                                [*width as usize, *height as usize],
                                &image_data,
                            );

                            let texture: egui::TextureHandle = ctx.load_texture(
                                "thumbnail",
                                color_image,
                                egui::TextureOptions::LINEAR,
                            );
                            ui.add_enabled_ui(true, |ui| {
                                item_card_image(
                                    ui,
                                    &texture,
                                    &mut i.get_pined(),
                                    self.settings.click_on_quit,
                                    i,
                                    &mut self.changed,
                                    path,
                                    ctx,
                                )
                            });
                        }
                    }
                    ui.label("");
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
