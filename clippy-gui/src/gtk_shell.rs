use std::cmp::Reverse;
use std::path::PathBuf;
use std::{fs, process};

use clippy::{Data, get_image_path, get_path};
use clippy_gui::{copy_to_linux, str_formate};

use adw::prelude::*;
use adw::{ActionRow, ApplicationWindow, HeaderBar};
use gtk::gdk::{self};
use gtk::{Application, Box, ListBox, Orientation, ScrolledWindow};
use gtk::{Image, Label};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use libadwaita as adw;

pub fn run_clip() {
    let application = Application::builder()
        .application_id("com.clippy.popup")
        .build();

    application.connect_startup(|_| {
        adw::init().unwrap();
    });

    application.connect_activate(|app| {
        // ActionRows are only available in Adwaita

        let data = get_data().unwrap();

        let list = ListBox::builder()
            .margin_top(10)
            .margin_end(10)
            .margin_bottom(10)
            .margin_start(10)
            .css_classes(vec![String::from("content")])
            .build();

        for (thumbnail, data) in data.1 {
            match thumbnail {
                Thumbnail::Text(_path) => {
                    let label = str_formate(&data.get_data().unwrap());
                    let row = ActionRow::builder()
                        .activatable(true)
                        .selectable(false)
                        .title(label)
                        .build();
                    let data = data.get_data().unwrap();

                    row.connect_activated(move |_| {
                        copy_to_linux("text/plain;charset=utf-8".to_string(), data.clone());
                        process::exit(0);
                    });

                    list.append(&row);
                }
                Thumbnail::Image(path) => {
                    let path = if path.is_file() {
                        path
                    } else {
                        PathBuf::from("assets/rust.png")
                    };

                    let file = gio::File::for_path(path.as_path());
                    let texture = gdk::Texture::from_file(&file).unwrap();

                    let image = Image::from_paintable(Some(&texture));
                    image.set_pixel_size(64);

                    let label = Label::new(Some("Image"));
                    label.set_xalign(0.0);

                    let hbox = Box::new(Orientation::Horizontal, 12);
                    hbox.append(&image);
                    hbox.append(&label);

                    let row = ActionRow::builder()
                        .activatable(true)
                        .selectable(false)
                        .build();
                    row.set_child(Some(&hbox));

                    row.connect_activated(move |_| {
                        copy_to_linux(
                            "image/png".to_string(),
                            data.get_image_as_string().unwrap().to_string(),
                        );
                        process::exit(0);
                    });

                    list.append(&row);
                }
            }
        }

        let scrolled_window = ScrolledWindow::builder()
            .vexpand(true)
            .hexpand(true)
            .child(&list)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .build();

        let content = Box::new(Orientation::Vertical, 0);
        content.append(
            &HeaderBar::builder()
                .title_widget(&adw::WindowTitle::new("Clippy", ""))
                .build(),
        );

        content.append(&scrolled_window);
        content.append(&list);

        let count = list.observe_children().into_iter().count() as i32;

        let max_height = 600;
        let min_height = 100;
        let max_items = 10;

        let step = (max_height - min_height) / (max_items - 1);

        let raw_height = min_height + step * (count.saturating_sub(1));

        let height = std::cmp::min(raw_height + 50, max_height);

        let window = ApplicationWindow::builder()
            .application(app)
            .default_width(400)
            .default_height(height)
            .content(&content)
            .build();

        // #################################
        // Part that is specific to use gtk4-layer-shell begins

        // Before the window is first realized, set it up to be a layer surface
        window.init_layer_shell();

        // Display above normal windows
        window.set_layer(Layer::Top);

        // Push other windows out of the way
        window.auto_exclusive_zone_enable();

        // Anchors are if the window is pinned to each edge of the output
        let anchors = [
            (Edge::Left, false),
            (Edge::Right, false),
            (Edge::Top, false),
            (Edge::Bottom, false),
        ];

        for (anchor, state) in anchors {
            window.set_anchor(anchor, state);
        }
        // Part that is specific to use gtk4-layer-shell ends
        // #################################

        window.show();
    });

    application.run();
}

pub enum Thumbnail {
    Image(std::path::PathBuf),
    Text(std::path::PathBuf),
}

fn get_data() -> Option<(i32, Vec<(Thumbnail, Data)>)> {
    let mut temp = Vec::new();
    if let Ok(entries) = fs::read_dir(get_path()) {
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_unstable_by_key(|entry| Reverse(entry.path()));
        let max = entries.len() - 1;
        let mut count = 0;
        for (i, entry) in entries.iter().enumerate() {
            let path = entry.path();
            if path.is_file() {
                if let Ok(content) = fs::read_to_string(&path) {
                    count += 1;
                    match serde_json::from_str::<Data>(&content) {
                        Ok(file) => {
                            if file.typ.starts_with("image/") {
                                temp.push((Thumbnail::Image(get_image_path(entry)), file));
                            } else {
                                temp.push((Thumbnail::Text(path), file));
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to parse {}: {}", path.display(), e)
                        }
                    }
                }

                if count >= 20 || i == max {
                    return Some((1, temp));
                }
            }
        }
    }
    None
}
