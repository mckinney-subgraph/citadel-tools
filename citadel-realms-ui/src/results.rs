use std::rc::Rc;
use std::cell::{RefCell,RefMut};


use gtk::prelude::*;
use gtk::{IconSize};

use crate::realms::Entity;
use crate::{Result,Builder};

const UI: &str = include_str!("../data/result.ui");

#[derive(Debug,Copy,Clone,PartialEq)]
pub enum ResultType {
    ConfigRealm,
    Realm,
    Terminal,
    StopRealm,
    RestartRealm,
    UpdateRealmFS,
}

#[derive(Clone)]
struct ResultItem {
    entity: Entity,
    item: gtk::Box,
    style: gtk::StyleContext,
    result_type: ResultType,
}

impl ResultItem {

    pub fn create(result_type: ResultType, entity: &Entity, parent: &gtk::Box) -> Result<Self> {
        let entity = entity.clone();
        let builder = Builder::new(UI);
        let item = builder.get_box("item-entry")?;
        let icon = builder.get_image("item-icon")?;
        let name = builder.get_label("item-name")?;
        let desc = builder.get_label("item-description")?;

        if entity.is_realm() {
            name.set_text(entity.name());
        } else {
            name.set_text(&format!("{}-realmfs", entity.name()));
        }

        match result_type {
            ResultType::ConfigRealm => {
                icon.set_from_icon_name(Some("emblem-system"), IconSize::Dialog);
                desc.set_text("Configure Realm");
                if let Some(indices) = entity.match_indices() {
                    Self::highlight_indices(&name, indices);
                }
            },
            ResultType::Realm => {
                icon.set_from_icon_name(Some("computer"), IconSize::Dialog);
                icon.set_sensitive(entity.is_running());
                if let Some(indices) = entity.match_indices() {
                    Self::highlight_indices(&name, indices);
                }

                if entity.description().is_empty() {
                    unsafe {
                        desc.destroy();
                    }
                } else {
                    desc.set_text(entity.description());
                }
            },
            ResultType::Terminal => {
                desc.set_text("Open Terminal");
                icon.set_from_icon_name(Some("utilities-terminal"), IconSize::Dialog);
                icon.set_sensitive(entity.is_running());
                if let Some(indices) = entity.match_indices() {
                    Self::highlight_indices(&name, indices);
                }
            }
            ResultType::StopRealm => {
                desc.set_text("Stop Realm");
                icon.set_from_icon_name(Some("system-shutdown-symbolic"), IconSize::Dialog);
                if let Some(indices) = entity.match_indices() {
                    Self::highlight_indices(&name, indices);
                }
            }
            ResultType::RestartRealm => {
                desc.set_text("Restart Realm");
                icon.set_from_icon_name(Some("system-reboot-symbolic"), IconSize::Dialog);
                if let Some(indices) = entity.match_indices() {
                    Self::highlight_indices(&name, indices);
                }
            }

            ResultType::UpdateRealmFS => {
                desc.set_text("Update RealmFS");
                icon.set_from_icon_name(Some("drive-harddisk-symbolic"), IconSize::Dialog);
                if let Some(indices) = entity.match_indices() {
                    Self::highlight_indices(&name, indices);
                }

            }
        }

        parent.pack_start(&item, false, true, 0);
        let style = item.get_style_context();
        item.show_all();
        Ok(ResultItem { entity, item, style, result_type })
    }

    fn highlight_range(attrs: &pango::AttrList, start: u32, end: u32) {
        let mut a = pango::Attribute::new_foreground(40000, 40000, 40000).unwrap();
        a.set_start_index(start);
        a.set_end_index(end);
        attrs.insert(a);
    }

    fn indices_to_ranges(indices: &[usize]) -> Vec<(u32, u32)> {
        let mut ranges = Vec::new();
        if indices.is_empty() {
            return ranges;
        }

        let first = indices[0] as u32;
        let mut current = (first, first);

        for i in &indices[1..] {
            let idx = *i as u32;
            if current.1 + 1 == idx {
                current.1 = idx;
            } else {
                ranges.push(current);
                current = (idx, idx);
            }
        }
        ranges.push(current);
        ranges
    }

    fn highlight_indices(label: &gtk::Label, indices: &[usize]) {
        if indices.is_empty() {
            return;
        }
        let ranges = Self::indices_to_ranges(indices);
        let attrs = pango::AttrList::new();
        for (start, end) in ranges {
            Self::highlight_range(&attrs, start, end + 1);
        }
        LabelExt::set_attributes(label, Some(&attrs));
    }


    fn set_selected(&self) {
        self.style.add_class("selected");
    }

    fn set_unselected(&self) {
        self.style.remove_class("selected");
    }

    fn activate(&self, window: &gtk::Window) -> bool {
        match self.result_type {
            ResultType::Realm => self.entity.activate(),
            ResultType::Terminal => self.entity.open_terminal(),
            ResultType::StopRealm => self.entity.stop_realm(),
            ResultType::RestartRealm => self.entity.restart_realm(),
            ResultType::ConfigRealm => self.entity.config_realm(window),
            ResultType::UpdateRealmFS => self.entity.update_realmfs(),
        }
    }
}

struct ResultItems {
    items: Vec<ResultItem>,
    selected: Option<usize>,
}

impl ResultItems {
    fn new() -> Self {
        ResultItems {
            items: Vec::new(),
            selected: None,
        }
    }

    fn clear(&mut self, parentbox: &gtk::Box) {
        self.selected = None;
        for item in self.items.drain(..) {
            ContainerExt::remove(parentbox, &item.item);
        }
    }

    pub fn create_item(&mut self, rtype: ResultType, realm: &Entity, parent: &gtk::Box) -> Result<()> {
        let item = ResultItem::create(rtype, realm, parent)?;
        self.items.push(item);
        if self.selected.is_none() {
            self.select(0);
        }
        Ok(())
    }

    fn select(&mut self, idx: usize) {
        if let Some(selected) = self.selected {
            if let Some(item) = self.items.get(selected) {
                item.set_unselected();
            }
        }
        if let Some(item) = self.items.get(idx) {
            item.set_selected();
            self.selected = Some(idx);
        }
    }

    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn selection_down(&mut self) {
        if self.is_empty() {
            return;
        }
        let idx = match self.selected {
            Some(idx) => (idx + 1) % self.items.len(),
            None => 0
        };
        self.select(idx);
    }

    fn selection_up(&mut self) {
        if self.is_empty() {
            return;
        }
        let idx = match self.selected {
            Some(0) => self.items.len() - 1,
            Some(idx) => idx - 1,
            None => self.items.len() - 1,
        };
        self.select(idx);
    }

    fn activate_selected(&self, window: &gtk::Window) -> bool {
        if let Some(idx) = self.selected {
            if let Some(item) = self.items.get(idx) {
                return item.activate(window);
            }
        }
        false
    }
}

#[derive(Clone)]
pub struct ResultList {
    result_box: gtk::Box,
    items: Rc<RefCell<ResultItems>>,
}

impl ResultList {
    pub fn new(result_box: gtk::Box) -> Self {
        ResultList {
            result_box,
            items: Rc::new(RefCell::new(ResultItems::new())),
        }
    }

    fn items_mut(&self) -> RefMut<ResultItems> {
        self.items.borrow_mut()
    }

    pub fn clear_list(&self) {
        self.items_mut().clear(&self.result_box);
        self.result_box.set_margin_top(0);
        self.result_box.set_margin_bottom(0);
    }

    pub fn selection_down(&self) {
        self.items_mut().selection_down();
    }

    pub fn selection_up(&self) {
        self.items_mut().selection_up();
    }

    pub fn create_result_items(&self, rtype: ResultType, entities: Vec<Entity>) {
        for r in &entities {
            if let Err(err) = self.items.borrow_mut().create_item(rtype, r, &self.result_box) {
                println!("failed to create {:?} item for realm {}: {:?}", rtype, r.name(), err);
            }
        }
    }

    pub fn activate_selected(&self, window: &gtk::Window) -> bool {
        self.items.borrow().activate_selected(window)
    }
}
