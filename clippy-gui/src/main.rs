#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use clipboard_img_widget::item_card_image;
use clipboard_widget::item_card;
use clippy::{
    Data, LoginUserCred, NewUser, NewUserOtp, SystemTheam, UserSettings, get_global_update_bool,
    get_path, get_path_pending, is_valid_email, is_valid_username, log_eprintln,
    set_global_update_bool,
};
use clippy_gui::{Thumbnail, Waiting, str_formate};
use custom_egui_widget::toggle;
use eframe::{
    App, NativeOptions,
    egui::{CentralPanel, ScrollArea, ViewportBuilder},
    run_native,
};
use egui::{
    Align, Button, Frame, Layout, Margin, RichText, Sense, Stroke, TextEdit, TextStyle, Theme,
    TopBottomPanel, Vec2,
};
use http::{check_user, login, signin, signin_otp_auth};
use std::{
    cmp::Reverse,
    collections::HashMap,
    fs::{self},
    io::Error,
    path::PathBuf,
    process,
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
    data: HashMap<u32, Vec<(Thumbnail, Data, PathBuf, bool)>>,
    page: u32,
    changed: bool,
    settings: UserSettings,
    show_settings: bool,
    show_signin_window: bool,
    newuser: NewUser,
    key: String,
    otp: String,
    waiting: Arc<Mutex<Waiting>>,
    show_login_window: bool,
    show_createuser_window: bool,
    show_createuser_auth_window: bool,
    show_error: (bool, String),
    warn: Option<String>,
    show_data_popup: (bool, String, PathBuf),
    scrool_to_top: bool,
}

impl Clipboard {
    fn new() -> Self {
        let data = Self::get_data();

        Self {
            data,
            page: 1,
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
            show_createuser_auth_window: false,
            show_error: (false, String::from("")),
            warn: None,
            newuser: NewUser::new_signin(String::new(), String::new()),
            key: "".to_string(),
            otp: String::new(),
            waiting: Arc::new(Mutex::new(Waiting::None)),
            show_data_popup: (false, String::new(), PathBuf::new()),
            scrool_to_top: false,
        }
    }

    fn refresh(&mut self) {
        self.data = Self::get_data();
    }

    fn get_data() -> HashMap<u32, Vec<(Thumbnail, Data, PathBuf, bool)>> {
        let mut data = HashMap::new();
        let mut temp = Vec::new();

        let mut count = 0;
        let mut page = 1;

        if let Ok(entries) = fs::read_dir(get_path_pending()) {
            let mut entries: Vec<_> = entries.flatten().collect();
            entries.sort_unstable_by_key(|entry| Reverse(entry.path()));
            let max = if entries.len() != 0 {
                entries.len() - 1
            } else {
                0
            };

            for (i, entry) in entries.iter().enumerate() {
                let path = entry.path();
                if path.is_file() {
                    if let Ok(content) = fs::read_to_string(&path) {
                        count += 1;
                        match serde_json::from_str::<Data>(&content) {
                            Ok(file) => {
                                if file.typ.starts_with("image/") {
                                    if let Some(val) = file.get_image_thumbnail(&entry) {
                                        temp.push((Thumbnail::Image(val), file, path, true));
                                    }
                                } else {
                                    if let Some(val) = file.get_data() {
                                        temp.push((
                                            Thumbnail::Text(str_formate(&val)),
                                            file,
                                            path,
                                            true,
                                        ));
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to parse {}: {}", path.display(), e)
                            }
                        }
                    }

                    if count >= 20 {
                        data.insert(page, temp);
                        temp = vec![];
                        page += 1;
                        if page > 100 {
                            break;
                        }
                        count = 0;
                    } else if i == max {
                        break;
                    }
                }
            }
        }

        if let Ok(entries) = fs::read_dir(get_path()) {
            let mut entries: Vec<_> = entries.flatten().collect();
            entries.sort_unstable_by_key(|entry| Reverse(entry.path()));
            let max = if entries.len() != 0 {
                entries.len() - 1
            } else {
                0
            };

            for (i, entry) in entries.iter().enumerate() {
                let path = entry.path();
                if path.is_file() {
                    if let Ok(content) = fs::read_to_string(&path) {
                        count += 1;
                        match serde_json::from_str::<Data>(&content) {
                            Ok(file) => {
                                if file.typ.starts_with("image/") {
                                    if let Some(val) = file.get_image_thumbnail(&entry) {
                                        temp.push((Thumbnail::Image(val), file, path, false));
                                    }
                                } else {
                                    if let Some(val) = file.get_data() {
                                        temp.push((
                                            Thumbnail::Text(str_formate(&val)),
                                            file,
                                            path,
                                            false,
                                        ));
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to parse {}: {}", path.display(), e)
                            }
                        }
                    }

                    if count >= 20 {
                        data.insert(page, temp);
                        temp = vec![];
                        page += 1;
                        if page > 100 {
                            break;
                        }
                        count = 0;
                    } else if i == max {
                        data.insert(page, temp);
                        temp = vec![];
                        break;
                    }
                }
            }
        }

        if !temp.is_empty() {
            data.insert(page, temp);
        }

        data
    }
}
impl App for Clipboard {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(std::time::Duration::from_secs(1));

        let button_size = Vec2::new(100.0, 35.0);

        TopBottomPanel::top("header").show(ctx, |ui| {
            let available_width = ui.available_width();

            ui.allocate_ui(Vec2::new(available_width, 0.0), |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(10.0);

                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        Frame::group(ui.style())
                            .corner_radius(9)
                            .outer_margin(Margin::same(4))
                            .show(ui, |ui| {
                                let button_next = Button::new(RichText::new("⬅").size(15.0))
                                    .min_size(Vec2::new(20.0, 20.0))
                                    .corner_radius(50.0)
                                    .stroke(Stroke::new(
                                        1.0,
                                        ui.visuals().widgets.inactive.bg_fill,
                                    ));

                                if ui.add(button_next).on_hover_text("Previous page").clicked() {
                                    self.page -= 1;
                                    self.scrool_to_top = true;
                                }

                                ui.label(self.page.to_string());

                                let button_prev = Button::new(RichText::new("➡").size(15.0))
                                    .min_size(Vec2::new(20.0, 20.0))
                                    .corner_radius(50.0)
                                    .stroke(Stroke::new(
                                        1.0,
                                        ui.visuals().widgets.inactive.bg_fill,
                                    ));

                                if ui.add(button_prev).on_hover_text("Next page").clicked() {
                                    self.page += 1;
                                    self.scrool_to_top = true;
                                }
                            });
                    });

                    ui.add_space((available_width / 2.0) - 150.0);
                    ui.label(RichText::new("Clippy").size(40.0));

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(10.0);

                        let button = Button::new(RichText::new("⚙").size(20.0))
                            .min_size(Vec2::new(30.0, 30.0))
                            .corner_radius(50.0)
                            .stroke(Stroke::new(1.0, ui.visuals().widgets.inactive.bg_fill));

                        if ui.add(button).on_hover_text("Settings").clicked() {
                            self.show_settings = true;
                        }

                        ui.add_space(10.0);
                    });
                });
            });

            if self.show_settings {
                let mut open = true;
                egui::Window::new("Settings")
                    .open(&mut open)
                    .resizable(false)
                    .collapsible(false)
                    .fixed_pos(ctx.screen_rect().center() - egui::vec2(170.0, 270.0))
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
                                            log_eprintln!(self.settings.write());
                                        }

                                        ui.add_space(10.0);
                                    });
                                } else if self.show_error.0 {
                                    ui.label(RichText::new("😢").size(150.0));

                                    ui.label(RichText::new(&self.show_error.1).size(20.0).strong());
                                    ui.add_space(10.0);
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                egui::RichText::new("Cancel").size(16.0).strong(),
                                            )
                                            .min_size(button_size),
                                        )
                                        .clicked()
                                    {
                                        self.show_error = (false, String::new())
                                    }
                                    ui.add_space(10.0);
                                } else if self.show_signin_window {
                                    ui.label(RichText::new("😃").size(150.0).strong());

                                    ui.vertical_centered(|ui| {
                                        ui.add_space(10.0);

                                        ui.label(
                                            RichText::new("Enter your details").size(20.0).strong(),
                                        );

                                        ui.add_space(8.0);

                                        ui.label(RichText::new(
                                            "Username must be 3–20 characters \
                                                 long and contain only letters, numbers, \
                                                or underscores (no spaces or special symbols).",
                                        ));

                                        ui.add_space(8.0);

                                        ui.style_mut().override_text_style =
                                            Some(TextStyle::Heading);

                                        ui.add(
                                            TextEdit::singleline(&mut self.newuser.user)
                                                .vertical_align(Align::Center)
                                                .hint_text("enter the username"),
                                        );

                                        if let Some(val) = &self.warn {
                                            ui.colored_label(egui::Color32::RED, val);
                                        }

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
                                                let username = self.newuser.user.clone();
                                                if is_valid_username(&username) {
                                                    self.warn = None;
                                                    let wait = self.waiting.clone();

                                                    thread::spawn(move || {
                                                        println!("started");
                                                        let async_runtime = Runtime::new().unwrap();
                                                        let status =
                                                            async_runtime.block_on(async {
                                                                check_user(username).await
                                                            });
                                                        let mut wait_lock = wait.lock().unwrap();
                                                        *wait_lock = Waiting::CheckUser(status);
                                                    });
                                                } else {
                                                    self.warn =
                                                        Some(String::from("Invalid username"));
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
                                        });
                                        ui.add_space(10.0);
                                    });
                                } else if self.show_createuser_window {
                                    ui.label(RichText::new("😃").size(150.0).strong());

                                    ui.label(
                                        RichText::new("Enter your details").size(20.0).strong(),
                                    );
                                    ui.add_space(8.0);

                                    if let Some(email) = &mut self.newuser.email {
                                        ui.style_mut().override_text_style =
                                            Some(TextStyle::Heading);

                                        ui.add(
                                            TextEdit::singleline(email)
                                                .vertical_align(Align::Center)
                                                .hint_text("Enter the Email"),
                                        );
                                    }
                                    if let Some(val) = &self.warn {
                                        ui.colored_label(egui::Color32::RED, val);
                                    }
                                    ui.style_mut().override_text_style = None;

                                    ui.add_space(10.0);

                                    ui.horizontal(|ui| {
                                        let total_button_width =
                                            button_size.x * 2.0 + 20.0 + 2.0 * 35.0;
                                        let available_width = ui.available_width();
                                        let horizontal_padding =
                                            (available_width - total_button_width).max(0.0) / 2.0;

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
                                            let user = self.newuser.clone();
                                            if is_valid_email(&self.newuser.email.as_ref().unwrap())
                                            {
                                                let wait = self.waiting.clone();

                                                thread::spawn(move || {
                                                    let async_runtime = Runtime::new().unwrap();

                                                    let signin = async_runtime
                                                        .block_on(async { signin(user).await });
                                                    match signin {
                                                        Ok(val) => {
                                                            let mut wait_lock =
                                                                wait.lock().unwrap();
                                                            *wait_lock =
                                                                Waiting::SigninOTP(Some(val));
                                                        }
                                                        Err(err) => {
                                                            eprintln!("{}", err);
                                                            let mut wait_lock =
                                                                wait.lock().unwrap();
                                                            *wait_lock = Waiting::Signin(None);
                                                        }
                                                    }
                                                });
                                            } else {
                                                self.warn = Some(String::from("Invalid Email"))
                                            }
                                        }

                                        ui.add_space(10.0);
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
                                            self.show_createuser_window = false;
                                        }
                                    });
                                    ui.add_space(20.0);
                                } else if self.show_createuser_auth_window {
                                    ui.label(RichText::new("😃").size(150.0).strong());

                                    ui.label(
                                        RichText::new("Enter your details").size(20.0).strong(),
                                    );

                                    ui.add_space(8.0);

                                    ui.style_mut().override_text_style = Some(TextStyle::Heading);

                                    ui.add(
                                        TextEdit::singleline(&mut self.otp)
                                            .vertical_align(Align::Center)
                                            .hint_text("enter the OTP")
                                            .min_size(button_size),
                                    );

                                    ui.add_space(8.0);

                                    ui.add(
                                        TextEdit::singleline(&mut self.key)
                                            .vertical_align(Align::Center)
                                            .hint_text("enter the Password")
                                            .min_size(button_size),
                                    );

                                    ui.style_mut().override_text_style = None;

                                    ui.add_space(10.0);

                                    ui.horizontal(|ui| {
                                        let total_button_width =
                                            button_size.x * 2.0 + 20.0 + 2.0 * 35.0;
                                        let available_width = ui.available_width();
                                        let horizontal_padding =
                                            (available_width - total_button_width).max(0.0) / 2.0;

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
                                            let wait = self.waiting.clone();
                                            let user = NewUserOtp::new(
                                                self.newuser.user.clone(),
                                                self.newuser.email.clone().unwrap(),
                                                self.otp.clone(),
                                                self.key.clone(),
                                            );
                                            thread::spawn(move || {
                                                let async_runtime = Runtime::new().unwrap();

                                                let signin = async_runtime.block_on(async {
                                                    signin_otp_auth(user).await
                                                });
                                                match signin {
                                                    Ok(val) => {
                                                        let mut wait_lock = wait.lock().unwrap();
                                                        *wait_lock = Waiting::Signin(Some(val));
                                                    }
                                                    Err(err) => {
                                                        eprintln!("{}", err);
                                                        let mut wait_lock = wait.lock().unwrap();
                                                        *wait_lock = Waiting::Signin(None);
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
                                            self.show_createuser_auth_window = false;
                                        }
                                        ui.add_space(20.0);
                                    });
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
                                                let user = LoginUserCred::new(
                                                    self.newuser.user.clone(),
                                                    self.key.clone(),
                                                );
                                                let wait = self.waiting.clone();
                                                thread::spawn(move || {
                                                    let async_runtime = Runtime::new().unwrap();

                                                    let login_result = async_runtime
                                                        .block_on(async { login(&user).await });

                                                    match login_result {
                                                        Err(err) => {
                                                            eprintln!("error logging in {}", err);
                                                            let mut wait_lock =
                                                                wait.lock().unwrap();
                                                            *wait_lock = Waiting::Login(None);
                                                        }
                                                        Ok(val) => {
                                                            let mut wait_lock =
                                                                wait.lock().unwrap();
                                                            *wait_lock = Waiting::Login(Some(val));
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
                                            if ui
                                                .selectable_value(
                                                    &mut self.settings.theme,
                                                    SystemTheam::System,
                                                    "System",
                                                )
                                                .clicked()
                                            {
                                                log_eprintln!(self.settings.write());
                                                if let Some(theam) = ctx.system_theme() {
                                                    if theam == Theme::Dark {
                                                        ctx.set_visuals(egui::Visuals::dark());
                                                    } else {
                                                        ctx.set_visuals(egui::Visuals::light());
                                                    }
                                                } else {
                                                    ctx.set_visuals(egui::Visuals::dark());
                                                };
                                            };
                                            if ui
                                                .selectable_value(
                                                    &mut self.settings.theme,
                                                    SystemTheam::Light,
                                                    "Light",
                                                )
                                                .changed()
                                            {
                                                log_eprintln!(self.settings.write());
                                                ctx.set_visuals(egui::Visuals::light());
                                            };
                                            if ui
                                                .selectable_value(
                                                    &mut self.settings.theme,
                                                    SystemTheam::Dark,
                                                    "Dark",
                                                )
                                                .clicked()
                                            {
                                                log_eprintln!(self.settings.write());
                                                ctx.set_visuals(egui::Visuals::dark());
                                            };
                                        });
                                });
                            });

                            let label = "Limits the number of clipboards stored on your device. \
                            It is recommended to limit this because it can grow over time.";
                            if let Some(mut val) = self.settings.max_clipboard {
                                ui.horizontal(|ui| {
                                    ui.label("Limite clipboard cache").on_hover_text(label);
                                    ui.with_layout(Layout::bottom_up(Align::RIGHT), |ui| {
                                        if ui.add(toggle(&mut true)).changed() {
                                            self.settings.max_clipboard = None;
                                            log_eprintln!(self.settings.write());
                                        }
                                    });
                                });

                                ui.horizontal(|ui| {
                                    ui.label("Total no of clipboard");
                                    ui.with_layout(Layout::bottom_up(Align::RIGHT), |ui| {
                                        if ui
                                            .add(egui::Slider::new(&mut val, 10..=1000).text(""))
                                            .changed()
                                        {
                                            self.settings.max_clipboard = Some(val);
                                            log_eprintln!(self.settings.write());
                                        }
                                    });
                                });
                            } else {
                                ui.horizontal(|ui| {
                                    ui.label("Limite clipboard cache");
                                    ui.with_layout(Layout::bottom_up(Align::RIGHT), |ui| {
                                        if ui.add(toggle(&mut false)).changed() {
                                            self.settings.max_clipboard = Some(100);
                                            log_eprintln!(self.settings.write());
                                        }
                                    });
                                });
                            }
                            let note = "If enabled, copied images are also stored as thumbnails. \
                            If disabled, image previews won’t be shown in the app.";
                            ui.horizontal(|ui| {
                                ui.label("Store Image Thumbnails").on_hover_text(note);
                                ui.with_layout(Layout::bottom_up(Align::RIGHT), |ui| {
                                    if ui.add(toggle(&mut self.settings.store_image)).changed() {
                                        log_eprintln!(self.settings.write());
                                    }
                                });
                            });

                            let note = "Clicking any clipboard item will copy \
                     its content and close the app.";
                            ui.horizontal(|ui| {
                                ui.label("Click to Copy and Quit").on_hover_text(note);
                                ui.with_layout(Layout::bottom_up(Align::RIGHT), |ui| {
                                    if ui.add(toggle(&mut self.settings.click_on_quit)).changed() {
                                        log_eprintln!(self.settings.write());
                                    }
                                });
                            });

                            ui.horizontal(|ui| {
                                ui.label("Placeholder");
                                ui.with_layout(Layout::bottom_up(Align::RIGHT), |ui| {
                                    if ui.add(toggle(&mut true)).changed() {
                                        log_eprintln!(self.settings.write());
                                    }
                                });
                            });

                            if self.settings.is_login() {
                                let note = "Prevents your clipboard from \
                                syncing to your cloud account.";
                                ui.horizontal(|ui| {
                                    ui.label("Disable Sync").on_hover_text(note);
                                    ui.with_layout(Layout::bottom_up(Align::RIGHT), |ui| {
                                        if ui.add(toggle(&mut self.settings.disable_sync)).changed()
                                        {
                                            log_eprintln!(self.settings.write());
                                        }
                                    });
                                });

                                ui.horizontal(|ui| {
                                    ui.label("Placeholder");
                                    ui.with_layout(Layout::bottom_up(Align::RIGHT), |ui| {
                                        if ui.add(toggle(&mut true)).changed() {
                                            log_eprintln!(self.settings.write());
                                        }
                                    });
                                });
                            }
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
                            self.show_signin_window = false;
                            self.show_login_window = true;
                            *val = Waiting::None;
                        }
                        Waiting::CheckUser(Some(false)) => {
                            self.show_signin_window = false;
                            self.show_createuser_window = true;
                            *val = Waiting::None;
                        }
                        Waiting::Login(Some(usercred)) => {
                            self.settings.set_user(usercred.clone());
                            self.show_login_window = false;
                            log_eprintln!(self.settings.write());
                            *val = Waiting::None;
                        }
                        Waiting::Login(None) => {
                            self.show_error = (true, String::from("Authentication failed"));
                            *val = Waiting::None;
                        }
                        Waiting::CheckUser(None) => {
                            self.show_error = (true, String::from("Problem connectiong to server"));
                            *val = Waiting::None;
                        }
                        Waiting::Signin(None) => {
                            self.show_error = (true, String::from("Invalid OTP or Network error"));
                            *val = Waiting::None;
                        }
                        Waiting::Signin(Some(usercred)) => {
                            self.show_createuser_window = false;
                            self.show_createuser_auth_window = false;
                            self.settings.set_user(usercred.clone());
                            log_eprintln!(self.settings.write());
                            *val = Waiting::None;
                        }
                        Waiting::SigninOTP(Some(true)) => {
                            self.show_createuser_window = false;
                            self.show_createuser_auth_window = true;
                            *val = Waiting::None;
                        }
                        Waiting::SigninOTP(Some(false)) => {
                            self.show_error =
                                (true, String::from("Invalid Email or Network error"));
                            *val = Waiting::None;
                        }
                        Waiting::SigninOTP(None) => {
                            self.show_error = (true, String::from("Network error"));
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

        if self.changed || get_global_update_bool() {
            self.refresh();
            self.changed = false;
            set_global_update_bool(false);
        }

        CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    if self.scrool_to_top {
                        let top_rect = ui.allocate_response(Vec2::ZERO, Sense::hover()).rect;
                        ui.scroll_to_rect(top_rect, Some(Align::TOP));
                        self.scrool_to_top = false;
                    }

                    let data = self.data.get(&self.page).or_else(|| {
                        self.page = 1;
                        self.data.get(&self.page)
                    });

                    if let Some(data) = data {
                        for (thumbnail, i, path, sync) in data {
                            if let Some(dat) = i.get_data() {
                                ui.add_enabled_ui(true, |ui| {
                                    item_card(
                                        ui,
                                        &dat,
                                        thumbnail,
                                        &mut i.get_pined(),
                                        self.settings.click_on_quit,
                                        &mut self.show_data_popup,
                                        &mut self.changed,
                                        path,
                                        ctx,
                                        sync,
                                    )
                                });
                            } else if let Thumbnail::Image((image_data, (width, height))) =
                                thumbnail
                            {
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
                                        &mut self.changed,
                                        path,
                                        ctx,
                                        sync,
                                    )
                                });
                            }
                        }
                    }
                    ui.label("");
                });
            });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.show_settings == true {
                self.show_settings = false;
            } else {
                process::exit(0)
            };
        }
    }
}

fn setup() -> Result<(), Error> {
    #[cfg(target_os = "windows")]
    {
        use std::{ffi::OsString, process::Command};
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
        let mut sys = System::new_all();
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::everything(),
        );
        let clippy_running = sys
            .processes_by_name(&OsString::from("clippy.exe"))
            .next()
            .is_some();
        if !clippy_running {
            println!("Starting clippy service");
            let _ = Command::new("cmd")
                .args(["/C", "start", "", "C:\\Program Files\\clippy\\clippy.exe"])
                .spawn()?;
        } else {
            println!("Clippy is running!")
        }
    }
    Ok(())
}

fn main() -> Result<(), eframe::Error> {
    // this fn make sure the clippy service is running
    log_eprintln!(setup());

    let ui = Clipboard::new();

    let icon =
        eframe::icon_data::from_png_bytes(include_bytes!("../../assets/icons/clippy-32-32.png"))
            .expect("The icon data must be valid");

    let options = NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size(Vec2::new(600.0, 800.0))
            .with_app_id("org.dhanu.clippy")
            .with_icon(icon),
        ..Default::default()
    };

    run_native(
        "clippy",
        options,
        Box::new(|cc| {
            match ui.settings.theme {
                SystemTheam::Dark => cc.egui_ctx.set_theme(egui::Theme::Dark),
                SystemTheam::Light => cc.egui_ctx.set_theme(egui::Theme::Light),
                SystemTheam::System => (),
            }
            Ok(Box::new(ui))
        }),
    )
}
