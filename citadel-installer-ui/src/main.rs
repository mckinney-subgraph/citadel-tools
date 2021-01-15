#![allow(deprecated)]
#[macro_use] extern crate glib;
use gtk::prelude::*;
use gio::prelude::*;
use std::env::args;
mod ui;
mod builder;
mod error;
mod rowdata;
mod dbus_client;
use libcitadel::CommandLine;
use ui::Ui;

pub use error::{Result,Error};

fn main() {
    let application =
        gtk::Application::new(Some("com.subgraph.citadel-installer"), Default::default())
            .expect("Initialization failed...");

    application.connect_activate(|app| {
        if !(CommandLine::live_mode() || CommandLine::install_mode()) {
            let dialog = gtk::MessageDialog::new(
                None::<&gtk::Window>,
                gtk::DialogFlags::empty(),
                gtk::MessageType::Error,
                gtk::ButtonsType::Cancel,
                "Citadel Installer can only be run during install mode");
            dialog.run();
        } else {
            match Ui::build(app) {
                Ok(ui) => {
                    ui.assistant.show_all();
                    ui.start();
                },
                Err(err) => {
                    println!("Could not start application: {:?}", err);
                }
            }
        }
    });
    application.run(&args().collect::<Vec<_>>());
}
