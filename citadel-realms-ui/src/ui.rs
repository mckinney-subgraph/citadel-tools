
use gtk::prelude::*;
use gtk::StyleContext;
use gdk::ModifierType;
use gdk::keys::constants;

use crate::matcher::Matcher;
use crate::results::ResultList;
use crate::{Result,Builder};

const STYLE: &str = include_str!("../data/style.css");
const MAIN_UI: &str = include_str!("../data/main.ui");


#[derive(Clone)]
pub struct Ui {
    window: gtk::Window,
    window_size: (i32, i32),
    input: gtk::Entry,
    result_list: ResultList,
    matcher: Matcher,
}

impl Ui {
    pub fn run(&self) {
        gtk::main();
    }

    pub fn build() -> Result<Self> {
        let builder = Builder::new(MAIN_UI);
        let window = builder.get_window("main-window")?;
        let current = builder.get_label("current-realm")?;
        let input = builder.get_entry("input-entry")?;
        let result_box = builder.get_box("result-box")?;

        window.set_opacity(0.85);
        window.set_icon_name(Some("cs-privacy"));

        window.show_all();

        let window_size = window.get_size();

        let matcher = Matcher::new()?;
        if let Some(realm) = matcher.current_realm() {
            current.set_text(realm.name());
        } else {
            current.hide();
        }

        let result_list = ResultList::new(result_box);

        let ui = Ui {
            window, window_size, input, result_list, matcher,
        };
        ui.setup_signals();
        ui.setup_style();
        Ok(ui)
    }


    fn setup_signals(&self) {
        let ui = self.clone();
        self.input.connect_activate(move |_| { ui.on_activate() });
        let ui = self.clone();
        self.input.connect_changed(move |e| {
            let s = e.get_text();
            ui.on_entry_changed(s.to_string().as_str());
        });
        let ui = self.clone();
        self.input.connect_key_press_event(move |_,k| {
            ui.on_key_press(k);
            Inhibit(false)
        });


        /*
        self.window.connect_focus_out_event(move |_,_| {
            gtk::idle_add(|| {
                gtk::main_quit();
                Continue(false)
            });
            Inhibit(false)

        });

         */

    }

    fn setup_style(&self) {
        if let Some(settings) = gtk::Settings::get_default() {
            settings.set_property_gtk_application_prefer_dark_theme(true);
        }
        let css = gtk::CssProvider::new();

        if let Err(err) = css.load_from_data(STYLE.as_bytes()) {
            println!("Error parsing CSS style: {}", err);
            return;
        }
        if let Some(screen) = gdk::Screen::get_default() {
            StyleContext::add_provider_for_screen(&screen, &css, gtk::STYLE_PROVIDER_PRIORITY_USER);
        }
    }

    fn on_activate(&self) {
        if self.result_list.activate_selected(&self.window) {
            println!("activated");
            self.input.set_text("");
            glib::idle_add_local({
                let (w,h) = self.window_size;
                let window = self.window.clone();
                move || {
                    window.resize(w, h);
                    gtk::main_quit();
                    Continue(false)
                }
            });
        }
    }

    fn on_entry_changed(&self, text: &str) {
        self.matcher.update(text, &self.result_list);
        let (w,h) = self.window_size;
        self.window.resize(w, h);
    }

    fn is_escape_key(keyval: gdk::keys::Key, state: ModifierType) -> bool {
        keyval == constants::Escape ||
            (state == ModifierType::CONTROL_MASK && keyval.to_unicode().unwrap() == '[')
    }

    fn on_key_press(&self, key: &gdk::EventKey) {
        let state = key.get_state();
        let keyval = key.get_keyval();

        if Self::is_escape_key(key.get_keyval(), key.get_state()) {
            gtk::main_quit();
        }
        if keyval == constants::Up {
            self.result_list.selection_up();
        } else if keyval == constants::Down {
            self.result_list.selection_down();
        } else if state == ModifierType::CONTROL_MASK {
            match keyval.to_unicode().unwrap() {
                'n'|'j' => self.result_list.selection_down(),
                'p'|'k' => self.result_list.selection_up(),
                _ => {},
            }
        }
    }

}
