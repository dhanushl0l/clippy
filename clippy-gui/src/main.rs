use clippy::{Data, UserSettings, get_path, set_global_bool, write_clipboard};
use eframe::{
    App, NativeOptions,
    egui::{CentralPanel, ScrollArea, ViewportBuilder},
    run_native,
};
use egui::{Align, Button, Layout, RichText, Stroke, TopBottomPanel, Vec2};
use std::{
    cmp::Reverse,
    fs::{self},
};

struct Clipboard {
    data: Vec<(Option<(Vec<u8>, (u32, u32))>, Data)>,
    loaded: bool,
    settings: UserSettings,
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
                        ui.add_space(10.0); // right padding

                        let button = Button::new(RichText::new("⚙").size(20.0))
                            .min_size(Vec2::new(30.0, 30.0))
                            .corner_radius(50.0) //
                            .stroke(Stroke::new(1.0, ui.visuals().widgets.inactive.bg_fill)); // optional border

                        if ui.add(button).on_hover_text("Settings").clicked() {
                            println!("Settings clicked!");
                        }
                    });
                });
            });
        });
        CentralPanel::default().show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                for (image, i) in &self.data {
                    if let Some(dat) = i.get_data() {
                        if ui.button(&dat).clicked() {
                            set_global_bool(true);
                            #[cfg(not(target_os = "linux"))]
                            write_clipboard::push_to_clipboard("String".to_string(), dat).unwrap();

                            #[cfg(target_os = "linux")]
                            clippy_gui::copy_to_linux("text/plain;charset=utf-8".to_string(), dat);

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
