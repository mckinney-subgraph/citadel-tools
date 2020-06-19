
use std::time::Duration;
use std::rc::Rc;
use std::cell::RefCell;

use dbus::blocking::{Connection,Proxy};
use crate::{Result, Error, ConfigDialog};
use std::collections::HashMap;


#[derive(Clone,PartialEq)]
enum EntityType{
    Realm,
    RealmFS,
}

#[derive(Clone)]
pub struct Entity {
    realms: RefCell<Realms>,
    etype: EntityType,
    name: String,
    description: Option<String>,
    realmfs: Option<String>,
    flags: Option<usize>,
    match_score: i64,
    match_indices: Option<Vec<usize>>,
}

impl Entity {
    fn new_realm(realms: Realms, (name, description, realmfs, flags): (String, String, String, u8)) -> Self {
        Self::new(realms, EntityType::Realm, name, Some(description), Some(realmfs), Some(flags as usize))
    }

    fn new_realmfs(realms: Realms, name: String) -> Self {
        Self::new(realms, EntityType::RealmFS, name, None, None, None)
    }

    fn new(realms: Realms, etype: EntityType, name: String, description: Option<String>, realmfs: Option<String>, flags: Option<usize>) -> Self {
        let realms = RefCell::new(realms);
        let match_score = 0;
        let match_indices = None;
        Entity { realms, etype, name, description, realmfs, flags, match_score, match_indices }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> &str {
        self.description.as_ref().map(|s| s.as_str()).unwrap_or("")
    }

    fn has_flag(&self, flag: usize) -> bool {
        self.flags.map(|v| v & flag != 0).unwrap_or(false)
    }
    pub fn is_running(&self) -> bool {
        self.has_flag(0x01)
    }

    pub fn is_system_realm(&self) -> bool {
        self.has_flag(0x04)
    }

    pub fn is_current(&self) -> bool {
        self.has_flag(0x02)
    }

    pub fn is_realm(&self) -> bool {
        self.etype == EntityType::Realm
    }

    pub fn realmfs_list(&self) -> Vec<Entity> {
        self.realms.borrow().cached_realmfs.clone()
    }

    fn with_realm<F>(&self, f: F) -> bool
    where F: Fn(&str) -> Result<()>
    {
        if !self.is_realm() {
            return false;
        }
        if let Err(err) = f(self.name()) {
            println!("error calling dbus method: {:?}", err);
        }
        true
    }

    pub fn activate(&self) -> bool {
        self.with_realm(|name| self.realms.borrow().set_current_realm(name))
    }
    pub fn open_terminal(&self) -> bool {
        self.with_realm(|name| self.realms.borrow().open_terminal(name))
    }

    pub fn stop_realm(&self) -> bool {
        self.with_realm(|name| self.realms.borrow().stop_realm(name))
    }
    pub fn restart_realm(&self) -> bool {
        self.with_realm(|name| self.realms.borrow().restart_realm(name))
    }

    pub fn config_realm(&self, window: &gtk::Window) -> bool {
        if !self.is_realm() {
            return false;
        }
        let config = match self.realms.borrow().get_realm_config(self.name()) {
            Ok(config) => config,
            Err(err) => {
                println!("Error requesting realm config for {}: {:?}", self.name(), err);
                return false;
            }
        };
        let config: HashMap<String,String> = config.into_iter().collect();
        let _c = ConfigDialog::open(self, config, window);
        false
    }

    pub fn update_realmfs(&self) -> bool {
        if self.is_realm() {
            return false;
        }
        if let Err(err) = self.realms.borrow().update_realmfs(self.name()) {
            println!("error calling dbus method: {:?}", err);
        }
        true
    }


    pub fn clone_with_match_info(&self, score: i64, indices: Vec<usize>) -> Self {
        let mut e = self.clone();
        e.match_score = score;
        e.match_indices = Some(indices);
        e
    }

    pub fn match_indices(&self) -> Option<&[usize]> {
        self.match_indices.as_ref().map(|v| v.as_slice())
    }

    pub fn match_score(&self) -> i64 {
        self.match_score
    }

}



#[derive(Clone)]
pub struct Realms {
    conn: Rc<Connection>,
    cached_realms: Vec<Entity>,
    cached_realmfs: Vec<Entity>,
}

impl Realms {

    pub fn connect() -> Result<Self> {
        let conn = Connection::new_system().map_err(Error::Dbus)?;
        let conn = Rc::new(conn);
        let cached_realms = Vec::new();
        let cached_realmfs = Vec::new();
        Ok(Realms { conn, cached_realms, cached_realmfs })
    }

    pub fn current_realm(&self) -> Option<&Entity> {
        self.cached_realms.iter().find(|r| r.is_current())
    }


    fn with_proxy<'a>(&self) -> Proxy<'a, &Connection> {
        self.conn.with_proxy("com.subgraph.realms", 
                             "/com/subgraph/realms", 
                             Duration::from_millis(5000))
    }

    pub fn realms(&self) -> &[Entity] {
        &self.cached_realms
    }

    pub fn realmfs(&self) -> &[Entity] {
        &self.cached_realmfs
    }

    pub fn reload_realms(&mut self) -> Result<()> {
        let realms = self.list()?;
        self.cached_realms.clear();
        self.cached_realms.extend_from_slice(&realms);

        let realmfs = self.get_realmfs_list()?;
        self.cached_realmfs.clear();
        self.cached_realmfs.extend_from_slice(&realmfs);
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<Entity>> {
        let (list,): (Vec<(String, String, String, u8)>,) =  self.with_proxy().method_call("com.subgraph.realms.Manager", "List", ()).map_err(Error::Dbus)?;
        let realms = list.into_iter()
            .map(|(n,d,fs, f)| Entity::new_realm(self.clone(), (n,d,fs,f)))
            .collect();
        Ok(realms)
    }

    pub fn open_terminal(&self, realm: &str) -> Result<()> {
        self.with_proxy().method_call("com.subgraph.realms.Manager", "Terminal", (realm,))
            .map_err(Error::Dbus)?;
        Ok(())
    }

    pub fn stop_realm(&self, realm: &str) -> Result<()> {
        self.with_proxy().method_call("com.subgraph.realms.Manager", "Stop", (realm,))
            .map_err(Error::Dbus)?;
        Ok(())
    }

    pub fn restart_realm(&self, realm: &str) -> Result<()> {
        self.with_proxy().method_call("com.subgraph.realms.Manager", "Restart", (realm,))
            .map_err(Error::Dbus)?;
        Ok(())
    }

    pub fn set_current_realm(&self, realm: &str) -> Result<()> {
        self.with_proxy().method_call("com.subgraph.realms.Manager", "SetCurrent", (realm,))
            .map_err(Error::Dbus)?;
        Ok(())
    }

    pub fn update_realmfs(&self, realmfs: &str) -> Result<()> {
        self.with_proxy().method_call("com.subgraph.realms.Manager", "UpdateRealmFS", (realmfs,))
            .map_err(Error::Dbus)?;
        Ok(())
    }

    pub fn get_realm_config(&self, realm: &str) -> Result<Vec<(String,String)>> {
        let (config,): (Vec<(String,String)>,) =  self.with_proxy().method_call("com.subgraph.realms.Manager", "RealmConfig", (realm, ))
            .map_err(Error::Dbus)?;
        Ok(config)
    }

    pub fn get_realmfs_list(&self) -> Result<Vec<Entity>> {
        let (list,): (Vec<String>,) = self.with_proxy().method_call("com.subgraph.realms.Manager", "ListRealmFS", ())
            .map_err(Error::Dbus)?;
        Ok(list.into_iter().map(|name| Entity::new_realmfs(self.clone(), name)).collect())
    }
}
