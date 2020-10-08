
use gtk::prelude::*;
use crate::{Error, Result};

pub struct Builder {
    builder: gtk::Builder,
}

impl Builder {
    pub fn new(source: &str) -> Self {
        let builder = gtk::Builder::from_string(source);
        Builder { builder }
    }

    fn ok_or_err<T>(type_name: &str, name: &str, object: Option<T>) -> Result<T> {
        object.ok_or(Error::Builder(format!("failed to load {} {}", type_name, name)))
    }

    pub fn get_entry(&self, name: &str) -> Result<gtk::Entry> {
        Self::ok_or_err("GtkEntry", name, self.builder.get_object(name))
    }

    pub fn get_box(&self, name: &str) -> Result<gtk::Box> {
        Self::ok_or_err("GtkBox", name, self.builder.get_object(name))
    }



    pub fn get_label(&self, name: &str) -> Result<gtk::Label> {
        Self::ok_or_err("GtkLabel", name, self.builder.get_object(name))
    }

    pub fn get_listbox(&self, name: &str) -> Result<gtk::ListBox> {
        Self::ok_or_err("GtkListBox", name, self.builder.get_object(name))
    }

    pub fn get_progress_bar(&self, name: &str) -> Result<gtk::ProgressBar> {
        Self::ok_or_err("GtkProgressBar", name, self.builder.get_object(name))
    }

    pub fn get_textview(&self, name: &str) -> Result<gtk::TextView> {
        Self::ok_or_err("GtkTextView", name, self.builder.get_object(name))
    }

    pub fn get_scrolled_window(&self, name: &str) -> Result<gtk::ScrolledWindow> {
        Self::ok_or_err("GtkScrolledWindow", name, self.builder.get_object(name))
    }
}
