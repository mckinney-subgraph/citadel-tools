
use gtk::prelude::*;
use gdk::ModifierType;
use gdk::enums::key;
use crate::{Result,Builder};
use crate::realms::Entity;
use std::collections::HashMap;

static CONFIG_FLAGS: &[(&str, &str)] = &[
    ("use-gpu", "Use GPU in Realm"),
    ("use-wayland", "Use Wayland in Realm"),
    ("use-x11", "Use X11 in Realm"),
    ("use-sound", "Use Sound in Realm"),
    ("use-shared-dir", "Mount /Shared directory in Realm"),
    ("use-network", "Realm has network access"),
    ("use-kvm", "Use KVM (/dev/kvm) in Realm"),
    ("use-ephemeral-home", "Use ephemeral tmpfs mount for home directory"),
];

const CONFIG_DIALOG: &str = include_str!("../data/config-dialog.ui");
const CONFIG_OPTION: &str = include_str!("../data/config-option.ui");

#[allow(dead_code)]
struct ConfigOption {
    name: &'static str,
    option: gtk::Box,
    check: gtk::CheckButton,
    style: gtk::StyleContext,
}

impl ConfigOption {
    fn create(name: &'static str, description: &str, val: bool) -> Result<Self> {
        let builder = Builder::new(CONFIG_OPTION);
        let option = builder.get_box("config-option")?;

        let check = builder.get_check_button("config-option-check")?;
        check.set_active(val);
        let label = builder.get_label("config-option-label")?;
        label.set_text(description);
        let style = option.get_style_context();
        Ok(ConfigOption { name, option, check, style })
    }
}

#[allow(dead_code)]
pub struct ConfigDialog {
    options: Vec<ConfigOption>,
}

impl ConfigDialog {

    pub fn open(realm: &Entity, config: HashMap<String,String>, parent: &gtk::Window) -> Result<Self> {
        let builder = Builder::new(CONFIG_DIALOG);
        let window = builder.get_window("config-dialog")?;
        let option_list = builder.get_box("config-option-list")?;
        let name_label = builder.get_label("config-realm-name")?;

        name_label.set_text(realm.name());
        window.set_decorated(false);

        let mut options = Vec::new();
        for (name,desc) in CONFIG_FLAGS {
            let val = match config.get(*name).map(|s| s.as_str()) {
                Some("true") => true,
                Some("false") => false,
                _ => false,
            };
            let option = ConfigOption::create(name, desc, val)?;
            option_list.pack_start(&option.option, false, false, 5);
            options.push(option);
        }

        let overlay = builder.get_combo_box_text("config-overlay-combo")?;
        println!("config: {:?}", config);
        let overlay_id  = match config.get("overlay").map(|s| s.as_str()) {
            Some("tmpfs") => "overlay-tmpfs",
            Some("storage") => "overlay-storage",
            _ => "overlay-none"
        };
        overlay.set_active_id(Some(overlay_id));

        let realmfs = builder.get_combo_box_text("config-realmfs-combo")?;
        for fs in realm.realmfs_list() {
            println!("adding {}", fs.name());
            // realmfs.append(Some(fs.name()), fs.name());
            ComboBoxTextExt::append(&realmfs, Some(fs.name()), fs.name());
        }

        let scheme = builder.get_button("theme-choose-button")?;
        if let Some(name) = config.get("terminal-scheme") {
            // scheme.set_label(name);
            ButtonExt::set_label(&scheme, name);
        }

        window.set_opacity(0.85);
        window.set_transient_for(Some(parent));
        parent.hide();
        window.show_all();
        window.connect_key_press_event({
            let win = window.clone();
            let parent = parent.clone();
            move |_,key| {
                let state = key.get_state();
                let keyval = key.get_keyval();
                let esc = keyval == key::Escape ||
                    (state == ModifierType::CONTROL_MASK && keyval == '[' as u32);
                if esc {
                    parent.show();
                    win.destroy();
                }
                Inhibit(false)

            }
        });

        Ok(ConfigDialog { options })
    }
}

