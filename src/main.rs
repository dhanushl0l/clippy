use gtk::{
    Application, ApplicationWindow, Box, Button, Label, ScrolledWindow, Stack, StackSwitcher,
};
use gtk::{glib, prelude::*};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Serialize)]
struct Data {
    data: Vec<u8>,
    typ: String,
    device: String,
}

fn read_json_files(directory: &str) -> Vec<Data> {
    let mut results = Vec::new();
    let path = Path::new(directory);

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(content) = fs::read_to_string(entry.path()) {
                if let Ok(parsed) = serde_json::from_str::<Data>(&content) {
                    results.push(parsed);
                }
            }
        }
    }
    results
}

fn build_ui(application: &Application) {
    let window = ApplicationWindow::new(application);
    window.set_title(Some("GTK JSON Viewer"));
    window.set_default_size(800, 600);

    let stack = Stack::new();
    let switcher = StackSwitcher::new();
    switcher.set_stack(Some(&stack));

    let button_page1 = Button::with_label("Go to Page 2");
    let button_page2 = Button::with_label("Go to Page 1");

    button_page1.add_css_class("flat");
    button_page1.add_css_class("no-radius");
    button_page2.add_css_class("flat");
    button_page2.add_css_class("no-radius");

    let header_box = Box::new(gtk::Orientation::Horizontal, 0);
    header_box.set_hexpand(true);
    header_box.set_vexpand(false);

    button_page1.set_hexpand(true);
    button_page2.set_hexpand(true);

    header_box.append(&button_page1);
    header_box.append(&button_page2);

    let page1 = Box::new(gtk::Orientation::Vertical, 10);
    let scrollable1 = ScrolledWindow::new();
    scrollable1.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    scrollable1.set_vexpand(true);

    page1.append(&scrollable1);

    let page2 = Box::new(gtk::Orientation::Vertical, 10);
    let scrollable2 = ScrolledWindow::new();
    scrollable2.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    scrollable2.set_vexpand(true);

    page2.append(&scrollable2);

    let data_list = read_json_files("/home/dhanu/.local/share/clippy/data");
    let page2_inner = Box::new(gtk::Orientation::Vertical, 10);

    for data in data_list {
        let truncated_text = String::from_utf8_lossy(&data.data);
        let display_text = if truncated_text.len() > 30 {
            format!("{}...", &truncated_text[..30])
        } else {
            truncated_text.to_string()
        };

        let label = Label::new(Some(&format!("{}", display_text)));
        label.set_margin_top(10);
        label.set_margin_bottom(10);
        label.set_margin_start(20);
        label.set_margin_end(20);
        label.set_css_classes(&["highlight"]);
        label.set_width_request(200);
        label.set_height_request(30);
        page2_inner.append(&label);
    }
    scrollable2.set_child(Some(&page2_inner));

    button_page1.connect_clicked(glib::clone!(
        #[weak]
        stack,
        move |_| {
            stack.set_visible_child_name("page2");
        }
    ));

    button_page2.connect_clicked(glib::clone!(
        #[weak]
        stack,
        move |_| {
            stack.set_visible_child_name("page1");
        }
    ));

    stack.add_named(&page1, Some("page1"));
    stack.add_named(&page2, Some("page2"));

    let vbox = Box::new(gtk::Orientation::Vertical, 5);
    vbox.append(&header_box);
    vbox.append(&stack);
    vbox.set_hexpand(true);
    vbox.set_vexpand(true);

    window.set_child(Some(&vbox));
    window.present();
}

fn main() {
    let application = Application::new(Some("com.example.gtkjson"), Default::default());
    application.connect_activate(build_ui);
    application.run();
}
