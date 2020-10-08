
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

    pub fn get_window(&self, name: &str) -> Result<gtk::Window> {
        Self::ok_or_err("GtkWindow", name, self.builder.get_object(name))
    }

    pub fn get_entry(&self, name: &str) -> Result<gtk::Entry> {
        Self::ok_or_err("GtkEntry", name, self.builder.get_object(name))
    }

    pub fn get_box(&self, name: &str) -> Result<gtk::Box> {
        Self::ok_or_err("GtkBox", name, self.builder.get_object(name))
    }

    pub fn get_check_button(&self, name: &str) -> Result<gtk::CheckButton> {
        Self::ok_or_err("GtkCheckButton", name, self.builder.get_object(name))
    }

    pub fn get_button(&self, name: &str) -> Result<gtk::Button> {
        Self::ok_or_err("GtkButton", name, self.builder.get_object(name))
    }

    pub fn get_grid(&self, name: &str) -> Result<gtk::Grid> {
        Self::ok_or_err("GtkGrid", name, self.builder.get_object(name))
    }


    pub fn get_label(&self, name: &str) -> Result<gtk::Label> {
        Self::ok_or_err("GtkLabel", name, self.builder.get_object(name))
    }

    pub fn get_image(&self, name: &str) -> Result<gtk::Image> {
        Self::ok_or_err("GtkImage", name, self.builder.get_object(name))
    }

    pub fn get_combo_box_text(&self, name: &str) -> Result<gtk::ComboBoxText> {
        Self::ok_or_err("GtkComboBoxText", name, self.builder.get_object(name))
    }
}
