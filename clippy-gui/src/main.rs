#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use clipboard_img_widget::item_card_image;
use clipboard_widget::item_card;
use clippy::{
    APP_ID, Data, LoginUserCred, NewUser, NewUserOtp, SystemTheam, UserSettings,
    get_global_update_bool, get_path, get_path_pending, get_path_pined, is_valid_email,
    is_valid_otp, is_valid_password, is_valid_username, log_error, set_global_update_bool,
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
use env_logger::{Builder, Env};
use http::{check_user, login, signin, signin_otp_auth};
use log::{debug, error};
use std::{
    fs::{self},
    io::Error,
    path::PathBuf,
    process,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};
use tokio::runtime::Runtime;

use crate::ipc::ipc::{init_stream, send_process};
mod clipboard_img_widget;
mod clipboard_widget;
mod custom_egui_widget;
mod edit_window;
mod http;
mod ipc;

struct Clipboard {
    page: PatgeData,
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
    show_data_popup: (bool, String, Option<PathBuf>, bool),
    scrool_to_top: bool,
}

#[derive(PartialEq)]
pub enum Page {
    Clipboard,
    Notification,
    Pined,
}

pub enum GETPAGE {
    NEXT,
    PREVIOUS,
    REFRESH,
}

pub struct PatgeData {
    page_no: u32,
    page_data: Option<Vec<(Thumbnail, PathBuf, Data, bool)>>,
    current_pos: Vec<u32>,
    current_patge: Page,
    data: Vec<(PathBuf, bool)>,
}

impl PatgeData {
    pub fn get_data() -> Vec<(PathBuf, bool)> {
        let mut temp = Vec::new();

        if let Ok(entries) = fs::read_dir(get_path()) {
            for entry in entries.flatten() {
                temp.push((entry.path(), false));
            }
        }

        if let Ok(entries) = fs::read_dir(get_path_pending()) {
            for entry in entries.flatten() {
                temp.push((entry.path(), true));
            }
        }

        if let Ok(entries) = fs::read_dir(get_path_pined()) {
            for entry in entries.flatten() {
                temp.push((entry.path(), false));
            }
        }

        fn priority(path: &PathBuf) -> u8 {
            let path_str = path.to_string_lossy();
            if path_str.contains("clippy/local_data/") {
                3
            } else if path_str.contains("clippy/data/") {
                2
            } else if path_str.contains("clippy/pined/") {
                1
            } else {
                0
            }
        }

        temp.sort_by(|(a, _), (b, _)| {
            let pa = priority(a);
            let pb = priority(b);

            pb.cmp(&pa).then_with(|| b.cmp(a))
        });
        temp
    }
}

impl Clipboard {
    fn new() -> Self {
        let mut new = Self {
            page: PatgeData {
                page_no: 1,
                page_data: None,
                current_pos: vec![0],
                current_patge: Page::Clipboard,
                data: PatgeData::get_data(),
            },
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
            show_data_popup: (false, String::new(), None, true),
            scrool_to_top: false,
        };
        new.get_current_page(GETPAGE::REFRESH);
        new
    }

    fn refresh(&mut self) {
        self.page.data = PatgeData::get_data();
        self.get_current_page(GETPAGE::REFRESH);
    }

    fn get_current_page(&mut self, get_page: GETPAGE) {
        let mut page_data = Vec::new();
        let mut count = 0;
        let mut current_pos = 0;
        let skip = match get_page {
            GETPAGE::NEXT => self.page.current_pos.last(),
            GETPAGE::PREVIOUS => {
                self.page.current_pos.pop();
                self.page.current_pos.pop();
                self.page.current_pos.last()
            }
            GETPAGE::REFRESH => {
                self.page.current_pos.pop();
                self.page.current_pos.last()
            }
        };
        for (i, path) in self
            .page
            .data
            .iter()
            .enumerate()
            .skip(*skip.unwrap_or(&0) as usize)
        {
            if let Ok(content) = fs::read_to_string(&path.0) {
                match serde_json::from_str::<Data>(&content) {
                    Ok(file) => match self.page.current_patge {
                        Page::Clipboard => {
                            if file.typ.starts_with("image/") {
                                if let Some(val) = file.get_image_thumbnail(&path.0) {
                                    page_data.push((
                                        Thumbnail::Image(val),
                                        path.0.clone(),
                                        file,
                                        path.1,
                                    ));
                                    count += 1;
                                }
                            } else {
                                if let Some(val) = file.get_meta_data() {
                                    page_data.push((
                                        Thumbnail::Text(val),
                                        path.0.clone(),
                                        file,
                                        path.1,
                                    ));
                                    count += 1;
                                }
                            }
                        }
                        Page::Notification => {
                            if file.typ.starts_with("notification/") {
                                if let Some(val) = file.get_meta_data() {
                                    page_data.push((
                                        Thumbnail::Text(val),
                                        path.0.clone(),
                                        file,
                                        path.1,
                                    ));
                                    count += 1;
                                }
                            }
                        }
                        Page::Pined => {
                            if file.pined {
                                if file.typ.starts_with("image/") {
                                    if let Some(val) = file.get_image_thumbnail(&path.0) {
                                        page_data.push((
                                            Thumbnail::Image(val),
                                            path.0.clone(),
                                            file,
                                            path.1,
                                        ));
                                        count += 1;
                                    }
                                } else {
                                    if let Some(val) = file.get_meta_data() {
                                        page_data.push((
                                            Thumbnail::Text(val),
                                            path.0.clone(),
                                            file,
                                            path.1,
                                        ));
                                        count += 1;
                                    }
                                }
                            }
                        }
                    },
                    Err(err) => {
                        eprintln!("json: {:?}", content);
                        eprintln!("{:?}", err)
                    }
                }
            }
            if i == 0 {
                self.page.page_no = 1;
            }
            if count == 10 || self.page.data.len() == i {
                current_pos = i + 1;
                break;
            }
        }
        self.page.current_pos.push(current_pos as u32);
        self.page.page_data = Some(page_data);
    }
}
impl App for Clipboard {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        log_error!(send_process(clippy::MessageIPC::Close));
    }
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        //if this the furst frame create a thread and monitor the clipboard & state changes
        if self.first_run {
            let ctxc = ctx.clone();
            let state = self.changed.clone();
            set_global_update_bool(false);
            std::thread::spawn(move || {
                loop {
                    thread::sleep(Duration::from_secs(1));
                    if get_global_update_bool() {
                        if let Ok(mut va) = state.try_lock() {
                            *va = true;
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
                                        if self.page.page_no > 1 {
                                            self.page.page_no -= 1;
                                            self.get_current_page(GETPAGE::PREVIOUS);
                                            self.scrool_to_top = true;
                                        }
                                    }

                                    ui.label(self.page.page_no.to_string());

                                    let button_prev = Button::new(RichText::new("➡").size(15.0))
                                        .min_size(Vec2::new(20.0, 20.0))
                                        .corner_radius(50.0)
                                        .stroke(Stroke::new(
                                            1.0,
                                            ui.visuals().widgets.inactive.bg_fill,
                                        ));

                                    if ui.add(button_prev).on_hover_text("Next page").clicked() {
                                        self.page.page_no += 1;
                                        self.get_current_page(GETPAGE::NEXT);
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

                            ui.add_space(1.0);
                            let button = Button::new(RichText::new("➕").size(20.0))
                                .min_size(Vec2::new(30.0, 30.0))
                                .corner_radius(50.0)
                                .stroke(Stroke::new(1.0, ui.visuals().widgets.inactive.bg_fill));

                            if ui.add(button).on_hover_text("Add notes").clicked() {
                                self.show_data_popup.0 = true;
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
                                                log_error!(send_process(clippy::MessageIPC::UpdateSettings(
                                                    self.settings.clone(),
                                                )));
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
                                                log_error!(send_process(clippy::MessageIPC::UpdateSettings(
                                                    self.settings.clone(),
                                                )));
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
                                                log_error!(send_process(clippy::MessageIPC::UpdateSettings(
                                                    self.settings.clone(),
                                                )));
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
                                                log_error!(send_process(clippy::MessageIPC::UpdateSettings(
                                                    self.settings.clone(),
                                                )));
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
                                                log_error!(send_process(clippy::MessageIPC::UpdateSettings(
                                                    self.settings.clone(),
                                                )));
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
                                                log_error!(send_process(clippy::MessageIPC::UpdateSettings(
                                                    self.settings.clone(),
                                                )));
                                        }
                                    });
                                });
                            } else {
                                ui.horizontal(|ui| {
                                    ui.label("Limite clipboard cache");
                                    ui.with_layout(Layout::bottom_up(Align::RIGHT), |ui| {
                                        if ui.add(toggle(&mut false)).changed() {
                                            self.settings.max_clipboard = Some(100);
                                                log_error!(send_process(clippy::MessageIPC::UpdateSettings(
                                                    self.settings.clone(),
                                                )));
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
                                                log_error!(send_process(clippy::MessageIPC::UpdateSettings(
                                                    self.settings.clone(),
                                                )));
                                    }
                                });
                            });

                            let note = "Show window in the top";
                                   ui.horizontal(|ui| {
                                       ui.label("Always on top").on_hover_text(note);
                                       ui.with_layout(Layout::bottom_up(Align::RIGHT), |ui| {
                                           if ui.add(toggle(&mut self.settings.always_on_top)).changed() {
                                            if self.settings.always_on_top{
                                            }
                                                       log_error!(send_process(clippy::MessageIPC::UpdateSettings(
                                                           self.settings.clone(),
                                                       )));
                                           }
                                       });
                                   });

                            let note = "Clicking any clipboard item will copy \
                     its content and close the app."; 
                            ui.horizontal(|ui| {
                                ui.label("Click to Copy and Quit").on_hover_text(note);
                                ui.with_layout(Layout::bottom_up(Align::RIGHT), |ui| {
                                    if ui.add(toggle(&mut self.settings.click_on_quit)).changed() {
                                                log_error!(send_process(clippy::MessageIPC::UpdateSettings(
                                                    self.settings.clone(),
                                                )));
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
                                                log_error!(send_process(clippy::MessageIPC::UpdateSettings(
                                                    self.settings.clone(),
                                                )));
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
                                                log_error!(send_process(clippy::MessageIPC::UpdateSettings(
                                                    self.settings.clone(),
                                                )));
                                        }
                                    });
                                });

                                ui.horizontal(|ui| {
                                    ui.label("Placeholder");
                                    ui.with_layout(Layout::bottom_up(Align::RIGHT), |ui| {
                                        if ui.add(toggle(&mut true)).changed() {
                                                log_error!(send_process(clippy::MessageIPC::UpdateSettings(
                                                    self.settings.clone(),
                                                )));
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
                                log_error!(send_process(clippy::MessageIPC::UpdateSettings(
                                    self.settings.clone(),
                                )));
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
                                log_error!(send_process(clippy::MessageIPC::UpdateSettings(
                                    self.settings.clone(),
                                )));
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

            if let Ok(mut va) = self.changed.clone().try_lock() {
                if *va {
                    self.refresh();
                    set_lock!(self.changed, false);
                    set_global_update_bool(false);
                    *va = false;
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
                        let data = &mut self.page.page_data;
                        if let Some(data) = data {
                            for (thumbnail, path, i, sync) in data.iter_mut() {
                                if let Thumbnail::Text(thumbnail) = thumbnail {
                                    ui.add_enabled_ui(true, |ui| {
                                        item_card(
                                            ui,
                                            i,
                                            thumbnail,
                                            &mut i.get_pined(),
                                            &self.settings,
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
                                            i,
                                            ui,
                                            &texture,
                                            &mut i.get_pined(),
                                            &self.settings,
                                            self.changed.clone(),
                                            path,
                                            ctx,
                                            sync,
                                        )
                                    });
                                }
                            }
                        }
                        ui.add_space(70.0);
                        ui.label("");
                    });
                    egui::Window::new("New pin")
                        .id(egui::Id::new("floating_button_window"))
                        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -10.0))
                        .resizable(false)
                        .collapsible(false)
                        .title_bar(false)
                        .frame(
                            egui::Frame::window(&ctx.style())
                                .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 0))
                                .stroke(egui::Stroke::NONE)
                                .shadow(egui::Shadow::NONE),
                        )
                        .show(ctx, |ui| {
                            ui.horizontal(|ui| {
                                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                    Frame::group(ui.style())
                                        .corner_radius(9.0)
                                        .fill(ui.visuals().window_fill())
                                        .outer_margin(Margin::same(10))
                                        .show(ui, |ui| {
                                            ui.vertical(|ui| {
                                                ui.horizontal(|ui| {
                                                    ui.add_space(11.0);
                                                    let button =
                                                        Button::new(RichText::new("📎").size(20.0))
                                                            .min_size(Vec2::new(30.0, 30.0))
                                                            .corner_radius(5.0)
                                                            .selected(
                                                                self.page.current_patge
                                                                    == Page::Clipboard,
                                                            )
                                                            .stroke(Stroke::new(
                                                                1.0,
                                                                ui.visuals()
                                                                    .widgets
                                                                    .inactive
                                                                    .bg_fill,
                                                            ));
                                                    if ui.add(button).clicked() {
                                                        self.page.current_patge = Page::Clipboard;
                                                        self.page.current_pos = vec![0];
                                                        self.page.page_no = 1;
                                                        self.get_current_page(GETPAGE::REFRESH);
                                                    }
                                                });
                                                ui.label("Clipboard")
                                            });
                                            ui.add_space(25.0);
                                            ui.vertical(|ui| {
                                                ui.horizontal(|ui| {
                                                    ui.add_space(16.0);
                                                    let button =
                                                        Button::new(RichText::new("🔔").size(20.0))
                                                            .min_size(Vec2::new(30.0, 30.0))
                                                            .corner_radius(5.0)
                                                            .selected(
                                                                self.page.current_patge
                                                                    == Page::Notification,
                                                            )
                                                            .stroke(Stroke::new(
                                                                1.0,
                                                                ui.visuals()
                                                                    .widgets
                                                                    .inactive
                                                                    .bg_fill,
                                                            ));
                                                    if ui.add(button).clicked() {
                                                        self.page.current_patge =
                                                            Page::Notification;
                                                        self.page.current_pos = vec![0];
                                                        self.page.page_no = 1;
                                                        self.get_current_page(GETPAGE::REFRESH)
                                                    }
                                                });
                                                ui.label("Notification")
                                            });
                                            ui.add_space(25.0);
                                            ui.vertical(|ui| {
                                                let button =
                                                    Button::new(RichText::new("📍").size(20.0))
                                                        .min_size(Vec2::new(30.0, 30.0))
                                                        .corner_radius(5.0)
                                                        .selected(
                                                            self.page.current_patge == Page::Pined,
                                                        )
                                                        .stroke(Stroke::new(
                                                            1.0,
                                                            ui.visuals().widgets.inactive.bg_fill,
                                                        ));
                                                if ui.add(button).clicked() {
                                                    self.page.current_patge = Page::Pined;
                                                    self.page.page_no = 1;
                                                    self.page.current_pos = vec![0];
                                                    self.get_current_page(GETPAGE::REFRESH);
                                                }
                                                ui.label("Pined")
                                            });
                                        });
                                });
                            });
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
                self.show_data_popup = (false, String::new(), None, false);
            } else {
                process::exit(0)
            };
        }
    }
}

fn setup() -> Result<(), Error> {
    Builder::from_env(Env::default().filter_or("LOG", "info")).init();
    if let Err(e) = init_stream() {
        error!(
            "clippy-gui cannot run without the Clippy backend. Please start `clippy`, not `clippy-gui`."
        );
        debug!("Details: {}", e);
        process::exit(1);
    }

    Ok(())
}

fn main() -> Result<(), eframe::Error> {
    // this fn make sure the clippy service is running
    log_error!(setup());

    let ui = Clipboard::new();

    let icon =
        eframe::icon_data::from_png_bytes(include_bytes!("../../assets/icons/clippy-32-32.png"))
            .expect("The icon data must be valid");

    let mut viewport = ViewportBuilder::default()
        .with_inner_size(Vec2::new(600.0, 800.0))
        .with_app_id(APP_ID)
        .with_icon(icon)
        .with_min_inner_size(Vec2::new(300.0, 300.0));

    viewport = if ui.settings.always_on_top {
        viewport.with_always_on_top()
    } else {
        viewport
    };

    let options = NativeOptions {
        viewport,
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
