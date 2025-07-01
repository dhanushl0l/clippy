#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use clipboard_img_widget::item_card_image;
use clipboard_widget::item_card;
use clippy::{
    APP_ID, Data, LoginUserCred, NewUser, NewUserOtp, SystemTheam, UserSettings,
    get_global_update_bool, get_path, get_path_pending, is_valid_email, is_valid_otp,
    is_valid_password, is_valid_username, log_eprintln, set_global_update_bool,
};
use clippy_gui::{Thumbnail, Waiting, set_lock};
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
    thread::{self, JoinHandle},
    time::Duration,
};
use tokio::runtime::Runtime;
mod clipboard_img_widget;
mod clipboard_widget;
mod custom_egui_widget;
mod edit_window;
mod http;

struct Clipboard {
    data: HashMap<u32, Vec<(PathBuf, bool)>>,
    page: (u32, Option<Vec<(Thumbnail, PathBuf, Data, bool)>>),
    changed: Arc<Mutex<bool>>,
    first_run: bool,
    settings: UserSettings,
    show_settings: bool,
    show_signin_window: bool,
    newuser: NewUser,
    key: String,
    otp: String,
    thread: Option<JoinHandle<()>>,
    waiting: Arc<Mutex<Waiting>>,
    show_login_window: bool,
    show_createuser_window: bool,
    show_createuser_auth_window: bool,
    show_error: (bool, String),
    warn: Option<String>,
    show_data_popup: (bool, String, PathBuf, bool),
    scrool_to_top: bool,
}

impl Clipboard {
    fn new() -> Self {
        let data = Self::get_data();
        let page = Self::get_current_page(&data, 1);

        Self {
            data,
            page: (1, page),
            changed: Arc::new(Mutex::new(false)),
            first_run: true,
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
            thread: None,
            waiting: Arc::new(Mutex::new(Waiting::None)),
            show_data_popup: (false, String::new(), PathBuf::new(), true),
            scrool_to_top: false,
        }
    }

    fn refresh(&mut self) {
        self.data = Self::get_data();
        let page = Self::get_current_page(&self.data, self.page.0);
        self.page.1 = page;
    }

    fn get_current_page(
        page: &HashMap<u32, Vec<(PathBuf, bool)>>,
        page_num: u32,
    ) -> Option<Vec<(Thumbnail, PathBuf, Data, bool)>> {
        let page = page.get(&page_num)?;
        let mut result = Vec::new();
        for path in page {
            if let Ok(content) = fs::read_to_string(&path.0) {
                match serde_json::from_str::<Data>(&content) {
                    Ok(file) => {
                        if file.typ.starts_with("image/") {
                            if let Some(val) = file.get_image_thumbnail(&path.0) {
                                result.push((Thumbnail::Image(val), path.0.clone(), file, path.1));
                            }
                        } else {
                            if let Some(val) = file.get_meta_data() {
                                result.push((Thumbnail::Text(val), path.0.clone(), file, path.1));
                            }
                        }
                    }
                    Err(err) => eprintln!("{:?}", err),
                }
            }
        }
        Some(result)
    }

    fn get_data() -> HashMap<u32, Vec<(PathBuf, bool)>> {
        let mut data = HashMap::new();
        let mut temp = Vec::new();

        let mut count = 0;
        let mut page = 1;

        if let Ok(entries) = fs::read_dir(get_path_pending()) {
            let mut entries: Vec<_> = entries.flatten().collect();
            entries.sort_unstable_by_key(|entry| Reverse(entry.path()));

            for entry in entries.iter() {
                let path = entry.path();
                if path.is_file() {
                    temp.push((path, true)); // or false in the second loop
                    count += 1;
                    if count >= 20 {
                        data.insert(page, temp);
                        temp = vec![];
                        page += 1;
                        if page > 100 {
                            break;
                        }
                        count = 0;
                    }
                }
            }
        }

        if let Ok(entries) = fs::read_dir(get_path()) {
            let mut entries: Vec<_> = entries.flatten().collect();
            entries.sort_unstable_by_key(|entry| Reverse(entry.path()));
            for entry in entries.iter() {
                let path = entry.path();
                if path.is_file() {
                    temp.push((path, false)); // or false in the second loop
                    count += 1;
                    if count >= 20 {
                        data.insert(page, temp);
                        temp = vec![];
                        page += 1;
                        if page > 100 {
                            break;
                        }
                        count = 0;
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
        //if this the furst frame create a thread and monitor the clipboard & state changes
        if self.first_run {
            let ctxc = ctx.clone();
            let state = self.changed.clone();
            std::thread::spawn(move || {
                loop {
                    thread::sleep(Duration::from_secs(1));
                    if get_global_update_bool() {
                        if let Ok(mut va) = state.try_lock() {
                            *va = true;
                        } else {
                        }
                        ctxc.request_repaint();
                    }
                }
            });
            self.first_run = false;
        }
        let button_size = Vec2::new(100.0, 35.0);
        if !self.show_data_popup.0 {
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

                                    if ui.add(button_next).on_hover_text("Previous page").clicked()
                                    {
                                        if self.page.0 > 1 {
                                            self.page.0 -= 1;
                                            self.refresh();
                                            self.scrool_to_top = true;
                                        }
                                    }

                                    ui.label(self.page.0.to_string());

                                    let button_prev = Button::new(RichText::new("➡").size(15.0))
                                        .min_size(Vec2::new(20.0, 20.0))
                                        .corner_radius(50.0)
                                        .stroke(Stroke::new(
                                            1.0,
                                            ui.visuals().widgets.inactive.bg_fill,
                                        ));

                                    if ui.add(button_prev).on_hover_text("Next page").clicked() {
                                        if self.data.contains_key(&(self.page.0 + 1)) {
                                            self.page.0 += 1;
                                            self.refresh();
                                            self.scrool_to_top = true;
                                        }
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

                                            ui.label(
                                                RichText::new("username:").size(12.3).strong(),
                                            );
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

                                        ui.label(
                                            RichText::new(&self.show_error.1).size(20.0).strong(),
                                        );
                                        ui.add_space(10.0);
                                        if ui
                                            .add(
                                                egui::Button::new(
                                                    egui::RichText::new("Close")
                                                        .size(16.0)
                                                        .strong(),
                                                )
                                                .min_size(button_size),
                                            )
                                            .clicked()
                                        {
                                            self.warn = None;
                                            self.show_error = (false, String::new())
                                        }
                                        ui.add_space(10.0);
                                    } else if self.show_signin_window {
                                        ui.label(RichText::new("😃").size(150.0).strong());

                                        ui.vertical_centered(|ui| {
                                            ui.add_space(10.0);

                                            ui.label(
                                                RichText::new("Enter your details")
                                                    .size(20.0)
                                                    .strong(),
                                            );

                                            ui.add_space(8.0);

                                            ui.label(RichText::new(
                                                "Username must be 3–20 characters \
                                                 long and contain only letters, numbers, \
                                                or underscores (no spaces,\
                                                 no uppercase letters or special symbols).",
                                            ));

                                            ui.add_space(8.0);
                                            if let Some(va) = &self.thread {
                                                if va.is_finished() {
                                                    self.thread = None
                                                } else {
                                                    ui.disable();
                                                }
                                            }

                                            ui.style_mut().override_text_style =
                                                Some(TextStyle::Heading);

                                            let response = ui.add(
                                                TextEdit::singleline(&mut self.newuser.user)
                                                    .vertical_align(Align::Center)
                                                    .hint_text("enter the username"),
                                            );

                                            let enter_pressed = response.lost_focus()
                                                && ui.input(|i| i.key_pressed(egui::Key::Enter));

                                            if let Some(val) = &self.warn {
                                                ui.colored_label(egui::Color32::RED, val);
                                            }

                                            ui.style_mut().override_text_style = None;

                                            ui.add_space(10.0);

                                            let total_button_width =
                                                button_size.x * 2.0 + 20.0 + 2.0 * 35.0;
                                            let available_width = ui.available_width();
                                            let horizontal_padding =
                                                (available_width - total_button_width).max(0.0)
                                                    / 2.0;

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
                                                    || enter_pressed
                                                {
                                                    let username = self.newuser.user.clone();
                                                    if is_valid_username(&username) {
                                                        self.warn = None;
                                                        let wait = self.waiting.clone();
                                                        let ctx = ctx.clone();
                                                        let thread = thread::spawn(move || {
                                                            let async_runtime =
                                                                Runtime::new().unwrap();
                                                            let status =
                                                                async_runtime.block_on(async {
                                                                    check_user(username).await
                                                                });
                                                            let mut wait_lock =
                                                                wait.lock().unwrap();
                                                            *wait_lock = Waiting::CheckUser(status);
                                                            ctx.request_repaint();
                                                        });
                                                        self.thread = Some(thread)
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
                                                    self.warn = None;
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

                                        let mut enter_pressed = false;
                                        if let Some(va) = &self.thread {
                                            if va.is_finished() {
                                                self.thread = None
                                            } else {
                                                ui.disable();
                                            }
                                        }

                                        if let Some(email) = &mut self.newuser.email {
                                            ui.style_mut().override_text_style =
                                                Some(TextStyle::Heading);

                                            let response = ui.add(
                                                TextEdit::singleline(email)
                                                    .vertical_align(Align::Center)
                                                    .hint_text("Enter the Email"),
                                            );

                                            enter_pressed = response.lost_focus()
                                                && ui.input(|i| i.key_pressed(egui::Key::Enter));
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
                                                (available_width - total_button_width).max(0.0)
                                                    / 2.0;

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
                                                || enter_pressed
                                            {
                                                let user = self.newuser.clone();
                                                if is_valid_email(
                                                    &self.newuser.email.as_ref().unwrap(),
                                                ) {
                                                    let wait = self.waiting.clone();
                                                    let ctx = ctx.clone();
                                                    let thread = thread::spawn(move || {
                                                        let async_runtime = Runtime::new().unwrap();

                                                        let signin = async_runtime
                                                            .block_on(async { signin(user).await });
                                                        let mut wait_lock = wait.lock().unwrap();
                                                        *wait_lock = Waiting::SigninOTP(signin);
                                                        ctx.request_repaint();
                                                    });
                                                    self.thread = Some(thread)
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
                                                self.warn = None;
                                                self.show_createuser_window = false;
                                            }
                                        });
                                        ui.add_space(10.0);
                                    } else if self.show_createuser_auth_window {
                                        ui.label(RichText::new("😃").size(150.0).strong());

                                        ui.label(
                                            RichText::new("Enter your details").size(20.0).strong(),
                                        );

                                        ui.add_space(8.0);

                                        ui.label(RichText::new(
                                            "Password must be 6–32 characters long,\
                                                and include at least one number, one symbol\
                                                and one uppercase letter.",
                                        ));

                                        ui.add_space(8.0);

                                        if let Some(va) = &self.thread {
                                            if va.is_finished() {
                                                self.thread = None
                                            } else {
                                                ui.disable();
                                            }
                                        }

                                        ui.style_mut().override_text_style =
                                            Some(TextStyle::Heading);

                                        let response = ui.add(
                                            TextEdit::singleline(&mut self.otp)
                                                .vertical_align(Align::Center)
                                                .hint_text("enter the OTP")
                                                .min_size(button_size),
                                        );

                                        let enter_pressed = response.lost_focus()
                                            && ui.input(|i| i.key_pressed(egui::Key::Enter));

                                        ui.add_space(8.0);

                                        let response = ui.add(
                                            TextEdit::singleline(&mut self.key)
                                                .vertical_align(Align::Center)
                                                .hint_text("enter the Password")
                                                .password(true)
                                                .min_size(button_size),
                                        );
                                        if enter_pressed {
                                            response.request_focus();
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
                                                (available_width - total_button_width).max(0.0)
                                                    / 2.0;

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
                                                || enter_pressed
                                            {
                                                let key = self.key.clone();
                                                let otp = self.otp.clone();
                                                if is_valid_otp(&key) {
                                                    if is_valid_password(&key) {
                                                        let wait = self.waiting.clone();
                                                        let user = NewUserOtp::new(
                                                            self.newuser.user.clone(),
                                                            self.newuser.email.clone().unwrap(),
                                                            otp,
                                                            key,
                                                        );
                                                        let ctx = ctx.clone();
                                                        let thread = thread::spawn(move || {
                                                            let async_runtime =
                                                                Runtime::new().unwrap();

                                                            let signin =
                                                                async_runtime.block_on(async {
                                                                    signin_otp_auth(user).await
                                                                });
                                                            let mut wait_lock =
                                                                wait.lock().unwrap();
                                                            *wait_lock = Waiting::Signin(signin);
                                                            ctx.request_repaint();
                                                        });
                                                        self.thread = Some(thread)
                                                    } else {
                                                        self.warn =
                                                            Some(String::from("Invalid password"));
                                                    }
                                                } else {
                                                    self.warn = Some(String::from("Invalid otp"));
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
                                                self.warn = None;
                                                self.show_createuser_auth_window = false;
                                            }
                                            ui.add_space(20.0);
                                        });
                                        ui.add_space(10.0);
                                    } else if self.show_login_window {
                                        ui.vertical_centered(|ui| {
                                            ui.label(RichText::new("😃").size(150.0).strong());

                                            ui.label(
                                                RichText::new("Enter your details")
                                                    .size(20.0)
                                                    .strong(),
                                            );

                                            ui.add_space(8.0);
                                            ui.label(
                                                RichText::new("Password:").size(15.0).strong(),
                                            );

                                            ui.style_mut().override_text_style =
                                                Some(TextStyle::Heading);

                                            ui.add_space(8.0);
                                            if let Some(va) = &self.thread {
                                                if va.is_finished() {
                                                    self.thread = None
                                                } else {
                                                    ui.disable();
                                                }
                                            }

                                            let response = ui.add(
                                                TextEdit::singleline(&mut self.key)
                                                    .vertical_align(Align::Center)
                                                    .hint_text("Enter the Password")
                                                    .password(true),
                                            );
                                            let enter_pressed = response.lost_focus()
                                                && ui.input(|i| i.key_pressed(egui::Key::Enter));
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
                                                    || enter_pressed
                                                {
                                                    let key = self.key.clone();
                                                    if is_valid_password(&key) {
                                                        let user = LoginUserCred::new(
                                                            self.newuser.user.clone(),
                                                            key,
                                                        );
                                                        let wait = self.waiting.clone();
                                                        let ctx = ctx.clone();
                                                        let thread = thread::spawn(move || {
                                                            let async_runtime =
                                                                Runtime::new().unwrap();

                                                            let status =
                                                                async_runtime.block_on(async {
                                                                    login(&user).await
                                                                });

                                                            let mut wait_lock =
                                                                wait.lock().unwrap();
                                                            *wait_lock = Waiting::Login(status);
                                                            ctx.request_repaint();
                                                        });
                                                        self.thread = Some(thread)
                                                    } else {
                                                        self.warn =
                                                            Some(String::from("Invalid password"));
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
                                                    self.warn = None;
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
                                                RichText::new("Enable Sync").size(24.0).strong(),
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
                                            .add(egui::Slider::new(&mut val, 30..=1000).text(""))
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

                            if self.settings.click_on_quit {
                                ui.horizontal(|ui| {
                                    ui.label("Paste on click");
                                    ui.with_layout(Layout::bottom_up(Align::RIGHT), |ui| {
                                        if ui
                                            .add(toggle(&mut self.settings.paste_on_click))
                                            .changed()
                                        {
                                            log_eprintln!(self.settings.write());
                                        }
                                    });
                                });
                            }

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
                            Waiting::CheckUser(Ok(true)) => {
                                self.show_signin_window = false;
                                self.show_login_window = true;
                                *val = Waiting::None;
                            }
                            Waiting::CheckUser(Ok(false)) => {
                                self.show_signin_window = false;
                                self.show_createuser_window = true;
                                *val = Waiting::None;
                            }
                            Waiting::Login(Ok(usercred)) => {
                                self.settings.set_user(usercred.clone());
                                self.show_login_window = false;
                                log_eprintln!(self.settings.write());
                                *val = Waiting::None;
                            }
                            Waiting::Login(Err(e)) => {
                                self.show_error = (true, e.to_string());
                                *val = Waiting::None;
                            }
                            Waiting::CheckUser(Err(e)) => {
                                self.show_error = (true, e.to_string());
                                *val = Waiting::None;
                            }
                            Waiting::Signin(Err(e)) => {
                                self.show_error = (true, e.into());
                                *val = Waiting::None;
                            }
                            Waiting::Signin(Ok(usercred)) => {
                                self.show_createuser_window = false;
                                self.show_createuser_auth_window = false;
                                self.settings.set_user(usercred.clone());
                                log_eprintln!(self.settings.write());
                                *val = Waiting::None;
                            }
                            Waiting::SigninOTP(Ok(_)) => {
                                self.show_createuser_window = false;
                                self.show_createuser_auth_window = true;
                                *val = Waiting::None;
                            }
                            Waiting::SigninOTP(Err(e)) => {
                                self.show_error = (true, e.to_string());
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
            });

            if let Ok(va) = self.changed.clone().try_lock() {
                if *va {
                    self.refresh();
                    set_lock!(self.changed, false);
                    set_global_update_bool(false);
                }
            }

            CentralPanel::default().show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ScrollArea::vertical().show(ui, |ui| {
                        if self.scrool_to_top {
                            let top_rect = ui.allocate_response(Vec2::ZERO, Sense::hover()).rect;
                            ui.scroll_to_rect(top_rect, Some(Align::TOP));
                            self.scrool_to_top = false;
                        }

                        let data = &self.page.1;

                        if let Some(data) = data {
                            for (thumbnail, path, i, sync) in data {
                                if let Some(dat) = i.get_data() {
                                    ui.add_enabled_ui(true, |ui| {
                                        item_card(
                                            ui,
                                            &dat,
                                            thumbnail,
                                            &mut i.get_pined(),
                                            self.settings.click_on_quit,
                                            &mut self.show_data_popup,
                                            self.changed.clone(),
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
                                            self.changed.clone(),
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
        } else {
            self.edit_window(ctx);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.show_settings {
                self.show_settings = false;
            } else if self.show_data_popup.0 {
                self.show_data_popup = (false, String::new(), PathBuf::new(), false);
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
            .with_app_id(APP_ID)
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
