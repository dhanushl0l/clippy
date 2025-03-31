use clippy::{Data, PATH};
use gtk::{
    Application, ApplicationWindow, Box, Button, Entry, ScrolledWindow, Stack, StackSwitcher,
};
use gtk::{glib, prelude::*};
use std::fs;
use std::path::{Path, PathBuf};

fn read_json_files(directory: PathBuf) -> Vec<Data> {
    let mut results = Vec::new();
    let path = Path::new(&directory);

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
    window.set_title(Some("Clippy"));
    window.set_default_size(400, 600);

    let stack = Stack::new();
    let switcher = StackSwitcher::new();
    switcher.set_stack(Some(&stack));

    let button_page1 = Button::with_label("clipboard");
    let button_page2 = Button::with_label("Pined");
    let button_page3 = Button::with_label("📍");

    button_page1.add_css_class("flat");
    button_page1.add_css_class("no-radius");
    button_page2.add_css_class("flat");
    button_page2.add_css_class("no-radius");
    button_page3.add_css_class("flat");
    button_page3.add_css_class("no-radius");

    let header_box = Box::new(gtk::Orientation::Horizontal, 0);
    header_box.set_hexpand(true);
    header_box.set_vexpand(false);

    button_page1.set_hexpand(true);
    button_page2.set_hexpand(true);

    header_box.append(&button_page1);
    header_box.append(&button_page2);
    header_box.append(&button_page3);

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

    let data_list = read_json_files(clippy::get_path(crate::PATH));
    let page1_inner = Box::new(gtk::Orientation::Vertical, 10);
    let page2_inner = Box::new(gtk::Orientation::Vertical, 10);

    for data in data_list {
        let truncated_text = data.get_data();
        let display_text = if truncated_text.len() > 30 {
            format!("{}...", &truncated_text[..30])
        } else {
            truncated_text
        };

        let label = Button::builder()
            .label(display_text)
            .margin_end(10)
            .margin_start(10)
            .build();

        if data.get_pined() {
            page1_inner.append(&label);
        } else {
            page2_inner.append(&label);
        }
    }

    scrollable2.set_child(Some(&page2_inner));
    scrollable1.set_child(Some(&page1_inner));

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

    button_page3.connect_clicked(|_| add_pin_window());

    stack.add_named(&page1, Some("page1"));
    stack.add_named(&page2, Some("page2"));

    let vbox = Box::new(gtk::Orientation::Vertical, 5);
    vbox.append(&header_box);
    vbox.append(&stack);

    window.set_child(Some(&vbox));
    window.present();
}

fn add_pin_window() {
    let window = ApplicationWindow::builder()
        .title("Add Pin")
        .default_width(300)
        .default_height(150)
        .build();

    let vbox = Box::new(gtk::Orientation::Vertical, 10);

    let entry = Entry::new();
    entry.set_placeholder_text(Some("Enter text..."));

    let button_box = Box::new(gtk::Orientation::Horizontal, 10);

    let save_button = Button::with_label("Save");
    save_button.connect_clicked(glib::clone!(
        #[weak]
        window,
        #[weak]
        entry,
        move |_| {
            let text = entry.text().to_string();
            if !text.is_empty() {
                let data = Data::new(text.into(), "pined".into(), "os".into(), true);
                // match data.write_to_json() {
                //     Ok(_) => (),
                //     Err(err) => eprintln!("{err}"),
                // };
            }
            window.close();
        }
    ));

    let cancel_button = Button::with_label("Cancel");
    cancel_button.connect_clicked(glib::clone!(
        #[weak]
        window,
        move |_| {
            window.close();
        }
    ));

    button_box.append(&save_button);
    button_box.append(&cancel_button);

    vbox.append(&entry);
    vbox.append(&button_box);

    window.set_child(Some(&vbox));

    window.present();
}

fn main() {
    let application = Application::new(Some("com.clippy.gtkapp"), Default::default());

    application.connect_activate(build_ui);
    application.run();
}
