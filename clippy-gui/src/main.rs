use clippy::{Data, get_path};
use eframe::{
    App, NativeOptions,
    egui::{CentralPanel, ScrollArea, ViewportBuilder},
    run_native,
};
use egui::Vec2;
use std::{cmp::Reverse, fs};

struct Clipboard {
    data: Vec<Data>,
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
                            Ok(file) => data.push(file),
                            Err(e) => {
                                eprintln!("Failed to parse {}: {}", entry.path().display(), e)
                            }
                        }
                    }
                }
            }
        }

        Self { data }
    }
}
impl App for Clipboard {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        CentralPanel::default().show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                for i in &self.data {
                    if let Some(dat) = i.get_data() {
                        let button = ui.button(dat);
                    }
                }
            })
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
