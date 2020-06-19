mod config;
mod error;
mod builder;
mod instance;
mod matcher;
mod realms;
mod results;
mod ui;

use ui::Ui;

pub use error::{Result,Error};
pub use builder::Builder;
pub use config::ConfigDialog;

fn main() {

    let tracker = match instance::InstanceTracker::create() {
        Ok(tracker) => tracker,
        Err(err) => {
            eprintln!("Failed to create instance tracker: {:?}", err);
            return;
        }
    };
    if !tracker.bind(true) {
        return;
    }
    if let Err(err) = gtk::init() {
        eprintln!("Failed to initialize GTK: {:?}", err);
        return;
    }
    let ui = match Ui::build() {
        Ok(ui) => ui,
        Err(err) => {
            eprintln!("Error: {:?}", err);
            return;
        }
    };
    ui.run();
}
