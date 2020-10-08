use gtk::prelude::*;

use gio::prelude::*;
use dbus::Message;
use std::time::Duration;
use std::thread;
use std::collections::HashMap;
use dbus::blocking::{Connection, Proxy};
use crate::builder::*;
use crate::rowdata::row_data::RowData;
use crate::{Result, Error};
use crate::dbus_client::*;

const STYLE: &str = include_str!("../data/style.css");
const WELCOME_UI: &str = include_str!("../data/welcome_page.ui");
const CITADEL_PASSWORD_UI: &str = include_str!("../data/citadel_password_page.ui");
const LUKS_PASSWORD_UI: &str = include_str!("../data/luks_password_page.ui");
const INSTALL_DESTINATION_UI: &str = include_str!("../data/install_destination_page.ui");
const CONFIRM_INSTALL_UI: &str = include_str!("../data/confirm_install_page.ui");
const INSTALL_UI: &str = include_str!("../data/install_page.ui");
pub enum Msg {
    InstallStarted,
    LvmSetup(String),
    LuksSetup(String),
    BootSetup(String),
    StorageCreated(String),
    RootfsInstalled(String),
    InstallCompleted,
    InstallFailed(String)
}
#[derive(Clone)]
pub struct Ui {
    pub assistant: gtk::Assistant,
    pub citadel_password_page: gtk::Box,
    pub citadel_password_entry: gtk::Entry,
    pub citadel_password_confirm_entry: gtk::Entry,
    pub citadel_password_status_label: gtk::Label,
    pub luks_password_page: gtk::Box,
    pub luks_password_entry: gtk::Entry,
    pub luks_password_confirm_entry: gtk::Entry,
    pub luks_password_status_label: gtk::Label,
    pub disks_listbox: gtk::ListBox,
    pub disks_model: gio::ListStore,
    pub confirm_install_label: gtk::Label,
    pub install_page: gtk::Box,
    pub install_progress: gtk::ProgressBar,
    pub install_scrolled_window: gtk::ScrolledWindow,
    pub install_textview: gtk::TextView,
    pub sender: glib::Sender<Msg>
}

impl Ui {
    pub fn build(application: &gtk::Application) -> Result<Self> {
        let disks = Self::get_disks()?;
        let assistant = gtk::Assistant::new();
        assistant.set_default_size(800, 600);
        assistant.set_position(gtk::WindowPosition::CenterAlways);
        
        assistant.set_application(Some(application));
        assistant.connect_delete_event(clone!(@strong application => move |_, _| {
            application.quit();
            gtk::Inhibit(false)
        }));
        assistant.connect_cancel(clone!(@strong application => move |_| {
            application.quit();
        }));
        let welcome_builder = Builder::new(WELCOME_UI);
        let welcome_page: gtk::Box = welcome_builder.get_box("welcome_page")?;
        let citadel_password_builder = Builder::new(CITADEL_PASSWORD_UI); 
        let citadel_password_page: gtk::Box = citadel_password_builder.get_box("citadel_password_page")?;
        let citadel_password_entry: gtk::Entry = citadel_password_builder.get_entry("citadel_password_entry")?;
        let citadel_password_confirm_entry: gtk::Entry = citadel_password_builder.get_entry("citadel_password_confirm_entry")?;
        let citadel_password_status_label: gtk::Label = citadel_password_builder.get_label("citadel_password_status_label")?;
        
        let luks_password_builder = Builder::new(LUKS_PASSWORD_UI);
        let luks_password_page: gtk::Box = luks_password_builder.get_box("luks_password_page")?;
        let luks_password_entry: gtk::Entry = luks_password_builder.get_entry("luks_password_entry")?;
        let luks_password_confirm_entry: gtk::Entry = luks_password_builder.get_entry("luks_password_confirm_entry")?;
        let luks_password_status_label: gtk::Label = luks_password_builder.get_label("luks_password_status_label")?;

        let install_destination_builder = Builder::new(INSTALL_DESTINATION_UI);
        let install_destination_page: gtk::Box = install_destination_builder.get_box("install_destination_page")?;
        let disks_listbox = install_destination_builder.get_listbox("install_destination_listbox")?;
        
        let confirm_install_builder = Builder::new(CONFIRM_INSTALL_UI);
        let confirm_install_page: gtk::Box = confirm_install_builder.get_box("confirm_install_page")?;
        let confirm_install_label: gtk::Label = confirm_install_builder.get_label("confirm_install_label_3")?;
        let disks_model = gio::ListStore::new(RowData::static_type());
        disks_listbox.bind_model(Some(&disks_model), move |item| {
            let row = gtk::ListBoxRow::new();
            let item = item.downcast_ref::<RowData>().expect("Row data is of wrong type");
            let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 5);
            hbox.set_homogeneous(true);
            let removable = item.get_property("removable").unwrap().get().unwrap().unwrap();
            let icon_name = Self::get_disk_icon(removable);
            let disk_icon = gtk::Image::from_icon_name(Some(&icon_name), gtk::IconSize::LargeToolbar);
            disk_icon.set_halign(gtk::Align::Start);
            let model_label = gtk::Label::new(None);
            model_label.set_halign(gtk::Align::Start);
            model_label.set_justify(gtk::Justification::Left);
            item.bind_property("model", &model_label, "label")
                .flags(glib::BindingFlags::DEFAULT | glib::BindingFlags::SYNC_CREATE)
                .build();
            let path_label = gtk::Label::new(None);
            path_label.set_halign(gtk::Align::Start);
            path_label.set_justify(gtk::Justification::Left);
            item.bind_property("path", &path_label, "label")
                .flags(glib::BindingFlags::DEFAULT | glib::BindingFlags::SYNC_CREATE)
                .build();
            let size_label = gtk::Label::new(None);
            size_label.set_halign(gtk::Align::Start);
            size_label.set_justify(gtk::Justification::Left);
            item.bind_property("size", &size_label, "label")
                .flags(glib::BindingFlags::DEFAULT | glib::BindingFlags::SYNC_CREATE)
                .build();
            hbox.pack_start(&disk_icon, true, true, 0);
            hbox.pack_start(&path_label, true, true, 0);
            hbox.pack_start(&model_label, true, true, 0);
            hbox.pack_start(&size_label, true, true, 0);
            row.add(&hbox);
            row.show_all();
            row.upcast::<gtk::Widget>()
        });
        disks_listbox.connect_row_selected(clone!(@strong assistant, @strong install_destination_page => move |_, listbox_row | {
            if let Some(_) = listbox_row {
                assistant.set_page_complete(&install_destination_page, true);
            }
        }));
        let install_builder = Builder::new(INSTALL_UI);
        let install_page: gtk::Box = install_builder.get_box("install_page")?;
        let install_progress: gtk::ProgressBar = install_builder.get_progress_bar("install_progress")?;
        let install_scrolled_window: gtk::ScrolledWindow = install_builder.get_scrolled_window("install_scrolled_window")?;
        let install_textview: gtk::TextView = install_builder.get_textview("install_textview")?;
        assistant.append_page(&welcome_page);
        assistant.set_page_type(&welcome_page, gtk::AssistantPageType::Intro);
        assistant.set_page_complete(&welcome_page, true);
        assistant.append_page(&citadel_password_page);
        assistant.append_page(&luks_password_page);
        assistant.append_page(&install_destination_page);
        assistant.append_page(&confirm_install_page);
        assistant.set_page_type(&confirm_install_page, gtk::AssistantPageType::Confirm);
        assistant.set_page_complete(&confirm_install_page, true);
        assistant.append_page(&install_page);
        assistant.set_page_type(&install_page, gtk::AssistantPageType::Progress);
        let disks_model_clone = disks_model.clone();
        let (sender, receiver) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        let ui = Self {
            assistant,
            citadel_password_page,
            citadel_password_entry,
            citadel_password_confirm_entry,
            citadel_password_status_label,
            luks_password_page,
            luks_password_entry,
            luks_password_confirm_entry,
            luks_password_status_label,
            disks_listbox,
            disks_model,
            confirm_install_label,
            install_page,
            install_progress,
            install_scrolled_window,
            install_textview,
            sender,
        };
        receiver.attach(None,clone!(@strong ui, @strong application =>  move |msg| {
            match msg {
                Msg::InstallStarted => {
                    ui.install_progress.set_fraction(0.1428);
                    let buffer = ui.install_textview.get_buffer().unwrap();
                    let mut iter = buffer.get_end_iter();
                    let text = format!(
                        "+ Installing Citadel to {}. \nFor a full log, consult the systemd journal by running the following command:\n <i>sudo journalctl -u citadel-installer-backend.service</i>\n", 
                        ui.get_install_destination());
                    buffer.insert_markup(&mut iter, &text);

                },
                Msg::LuksSetup(text) => {
                    ui.install_progress.set_fraction(0.1428 * 2.0);
                    let buffer = ui.install_textview.get_buffer().unwrap();
                    let mut iter = buffer.get_end_iter();
                    buffer.insert(&mut iter, &text);
                },
                Msg::LvmSetup(text) => {
                    ui.install_progress.set_fraction(0.1428 * 3.0);
                    let buffer = ui.install_textview.get_buffer().unwrap();
                    let mut iter = buffer.get_end_iter();
                    buffer.insert(&mut iter, &text);
                },
                Msg::BootSetup(text) => {
                    ui.install_progress.set_fraction(0.1428 * 4.0);
                    let buffer = ui.install_textview.get_buffer().unwrap();
                    let mut iter = buffer.get_end_iter();
                    buffer.insert(&mut iter, &text);
                },
                Msg::StorageCreated(text) => {
                    ui.install_progress.set_fraction(0.1428 * 5.0);
                    let buffer = ui.install_textview.get_buffer().unwrap();
                    let mut iter = buffer.get_end_iter();
                    buffer.insert(&mut iter, &text);
                },
                Msg::RootfsInstalled(text) => {
                    ui.install_progress.set_fraction(0.1428 * 6.0);
                    let buffer = ui.install_textview.get_buffer().unwrap();
                    let mut iter = buffer.get_end_iter();
                    buffer.insert(&mut iter, &text);
                },
                Msg::InstallCompleted => {
                    ui.install_progress.set_fraction(1.0);
                    let buffer = ui.install_textview.get_buffer().unwrap();
                    let mut iter = buffer.get_end_iter();
                    buffer.insert(&mut iter, "+ Completed the installation successfully\n");
                    let quit_button = gtk::Button::with_label("Quit");
                    quit_button.connect_clicked(clone!(@strong application => move |_| {
                        application.quit();
                    }));
                    quit_button.set_sensitive(true);
                    ui.assistant.add_action_widget(&quit_button);
                    ui.assistant.show_all();
                },
                Msg::InstallFailed(error) => {
                    ui.install_progress.set_fraction(100.0);
                    let buffer = ui.install_textview.get_buffer().unwrap();
                    let mut iter = buffer.get_end_iter();
                    let text = format!("+ Install failed with error:\n<i>{}</i>\n", error);
                    buffer.insert_markup(&mut iter, &text);
                    let quit_button = gtk::Button::with_label("Quit");
                    quit_button.connect_clicked(clone!(@strong application => move |_| {
                        application.quit();
                    }));
                    quit_button.set_sensitive(true);
                    ui.assistant.add_action_widget(&quit_button);
                    ui.assistant.show_all();
                } 
            }
            glib::Continue(true)
        }));
        ui.setup_style();
        ui.setup_signals();
        for disk in disks {
            disks_model_clone.append(&disk);
        }
        Ok(ui)
    }
    
    fn get_disks() -> Result<Vec<RowData>> {
        let mut disks = vec![];
        let conn = Connection::new_system().unwrap();
        let proxy = conn.with_proxy("com.subgraph.installer", 
        "/com/subgraph/installer", Duration::from_millis(5000));
        let (devices,): (HashMap<String, Vec<String>>,) = proxy.method_call("com.subgraph.installer.Manager", "GetDisks", ()).map_err(Error::Dbus)?;
            for device in devices {
                let disk = RowData::new(
                    &device.1[0].clone(), 
                    &device.0, 
                    &device.1[1].clone(), 
                    device.1[2].parse().unwrap());
                disks.push(disk);
            }
        Ok(disks)
    }

    fn get_disk_icon(removable: bool) -> String {
        if removable {
            return "drive-harddisk-usb-symbolic".to_string();
        }
        "drive-harddisk-system-symbolic".to_string()
    } 

    pub fn setup_entry_signals(&self, page: &gtk::Box, first_entry: &gtk::Entry, second_entry: &gtk::Entry, status_label: &gtk::Label) {
        let ui = self.clone();
        let assistant = ui.assistant.clone();
        first_entry.connect_changed(clone!(@weak assistant, @weak page, @weak second_entry, @weak status_label => move |entry| {
            let password = entry.get_text();
            let confirm = second_entry.get_text();
            if password != "" && confirm != "" {
                let matches = password == confirm;
                if !matches {
                    status_label.set_text("Passwords do not match");
                } else {
                    status_label.set_text("");
                }
                assistant.set_page_complete(&page, matches);
            }
        }));
        first_entry.connect_activate(clone!(@weak second_entry => move |_| {
            second_entry.grab_focus();
        }));
    }

    pub fn setup_prepare_signal(&self) {
        let ui = self.clone();
        ui.assistant.connect_prepare(clone!(@strong ui => move |assistant, page| {
            let page_type = assistant.get_page_type(page);
            if page_type == gtk::AssistantPageType::Confirm {
                let path = ui.get_install_destination();
                let text = format!("<i>{}</i>", path);
                ui.confirm_install_label.set_markup(&text);
            }
        }));
    }

    pub fn setup_apply_signal(&self) {
        let ui = self.clone();
        ui.assistant.connect_apply(clone!(@strong ui => move |_| {
            let citadel_password = ui.get_citadel_password();
            let luks_password = ui.get_luks_password();
            let destination = ui.get_install_destination();
            let conn = Connection::new_system().unwrap();
            let proxy = conn.with_proxy("com.subgraph.installer", 
                "/com/subgraph/installer", Duration::from_millis(5000));
            let (_,): (bool,) = proxy.method_call("com.subgraph.installer.Manager", 
                "RunInstall", (destination, citadel_password, luks_password)).unwrap();
            let _= ui.sender.send(Msg::InstallStarted);
        }));
    }

    pub fn setup_autoscroll_signal(&self) {
        let ui = self.clone();
        let scrolled_window = ui.install_scrolled_window;
        ui.install_textview.connect_size_allocate(clone!(@weak scrolled_window => move |_, _| {
            let adjustment = scrolled_window.get_vadjustment().unwrap();
            adjustment.set_value(adjustment.get_upper() - adjustment.get_page_size());
        }));
    }

    pub fn setup_signals(&self) {
        let ui = self.clone();
        self.setup_entry_signals(&ui.citadel_password_page, &ui.citadel_password_entry, 
            &ui.citadel_password_confirm_entry, &ui.citadel_password_status_label);
        self.setup_entry_signals(&ui.citadel_password_page, &ui.citadel_password_confirm_entry, 
            &ui.citadel_password_entry, &ui.citadel_password_status_label);
        self.setup_entry_signals(&ui.luks_password_page, &ui.luks_password_entry, 
            &ui.luks_password_confirm_entry, &ui.luks_password_status_label);
        self.setup_entry_signals(&ui.luks_password_page, &ui.luks_password_confirm_entry, 
            &ui.luks_password_entry, &ui.luks_password_status_label);
        self.setup_prepare_signal();
        self.setup_apply_signal();
        self.setup_autoscroll_signal();
    }

    fn setup_style(&self) {
        let css = gtk::CssProvider::new();

        if let Err(err) = css.load_from_data(STYLE.as_bytes()) {
            println!("Error parsing CSS style: {}", err);
            return;
        }
        if let Some(screen) = gdk::Screen::get_default() {
            gtk::StyleContext::add_provider_for_screen(&screen, &css, gtk::STYLE_PROVIDER_PRIORITY_USER);
        }
    }

    pub fn get_citadel_password(&self) -> String {
        let ui = self.clone();
        let password = ui.citadel_password_entry.get_text();
        password.to_string()
    }
    
    pub fn get_luks_password(&self) -> String {
        let ui = self.clone();
        let password = ui.luks_password_entry.get_text();
        password.to_string()
    }

    pub fn get_install_destination(&self) -> String {
        let ui = self.clone();
        let model = ui.disks_model;
        if let Some(row) = ui.disks_listbox.get_selected_row() {
            let index = row.get_index() as u32;
            let data = model.get_object(index).unwrap(); 
            let data = data.downcast_ref::<RowData>().expect("Row data is of wrong type");
            // TODO: Fix unwrap
            let path: String = data.get_property("path").unwrap().get().unwrap().unwrap();
            return path.to_string();
        }
        "".to_string()
    }
    fn setup_signal_matchers(&self, proxy: Proxy<&Connection>) {
        let sender = self.sender.clone();
        let _ = proxy.match_signal(clone!(@strong sender => move |_: ComSubgraphInstallerManagerInstallCompleted, _: &Connection, _: &Message| {
            let _ = sender.send(Msg::InstallCompleted);
            true
        }));
        let _ = proxy.match_signal(clone!(@strong sender => move |h: ComSubgraphInstallerManagerLvmSetup, _: &Connection, _: &Message| {
            let _ = sender.send(Msg::LvmSetup(h.text));
            true
        }));
        let _ = proxy.match_signal(clone!(@strong sender => move |h: ComSubgraphInstallerManagerLuksSetup, _: &Connection, _: &Message| {
            let _ = sender.send(Msg::LuksSetup(h.text));
            true
        }));
        let _ = proxy.match_signal(clone!(@strong sender => move |h: ComSubgraphInstallerManagerBootSetup, _: &Connection, _: &Message| {
            let _ = sender.send(Msg::BootSetup(h.text));
            true
        }));
        let _ = proxy.match_signal(clone!(@strong sender => move |h: ComSubgraphInstallerManagerStorageCreated, _: &Connection, _: &Message| {
            let _ = sender.send(Msg::StorageCreated(h.text));
            true
        }));
        let _ = proxy.match_signal(clone!(@strong sender => move |h: ComSubgraphInstallerManagerRootfsInstalled, _: &Connection, _: &Message| {
            let _ = sender.send(Msg::RootfsInstalled(h.text));
            true
        }));
        let _ = proxy.match_signal(clone!(@strong sender => move |h: ComSubgraphInstallerManagerInstallFailed, _: &Connection, _: &Message| {
            let _ = sender.send(Msg::InstallFailed(h.text));
            true
        }));
    }

    pub fn start(&self) {
        let c = Connection::new_system().unwrap();
        let proxy = c.with_proxy("com.subgraph.installer", "/com/subgraph/installer", Duration::from_millis(5000));
        self.setup_signal_matchers(proxy);
        thread::spawn(move || {
            loop { 
                c.process(Duration::from_millis(1000)).unwrap(); }
        });
    }
}
