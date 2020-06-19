use std::sync::Arc;
use std::{result, thread};

use dbus::tree::{self, Factory, MTFn, MethodResult, Tree, MethodErr};
use dbus::{Connection, NameFlag, Message};
use libcitadel::{Result, RealmManager, Realm, RealmEvent, OverlayType, RealmFS, terminal};
use std::fmt;

type MethodInfo<'a> = tree::MethodInfo<'a, MTFn<TData>, TData>;

// XXX
const UPDATE_TOOL_PATH: &str = "/realms/Shared/citadel-realmfs";
const SUDO_PATH: &str = "/usr/bin/sudo";

const STATUS_REALM_RUNNING: u8 = 1;
const STATUS_REALM_CURRENT: u8 = 2;
const STATUS_REALM_SYSTEM_REALM: u8  = 4;

const OBJECT_PATH: &str = "/com/subgraph/realms";
const INTERFACE_NAME: &str = "com.subgraph.realms.Manager";
const BUS_NAME: &str = "com.subgraph.realms";

const OBJECT_MANAGER_INTERFACE: &str = "org.freedesktop.DBus.ObjectManager";
const VPN_CONNECTION_INTERFACE: &str = "org.freedesktop.VPN.Connection";

pub struct DbusServer {
    connection: Arc<Connection>,
    manager: Arc<RealmManager>,
    events: EventHandler,
}

impl DbusServer {

    pub fn connect(manager: Arc<RealmManager>) -> Result<DbusServer> {
        let connection = Arc::new(Connection::get_private(dbus::BusType::System)?);
        let events = EventHandler::new(connection.clone());
        let server = DbusServer { events, connection, manager };
        Ok(server)
    }

    fn build_tree(&self) -> Tree<MTFn<TData>, TData> {
        let f = Factory::new_fn::<TData>();
        let data = TreeData::new(self.manager.clone());
        let interface = f.interface(INTERFACE_NAME, ())
            // Methods
            .add_m(f.method("SetCurrent", (), Self::do_set_current)
                .in_arg(("name", "s")))

            .add_m(f.method("GetCurrent", (), Self::do_get_current)
                .out_arg(("name", "s")))

            .add_m(f.method("List", (), Self::do_list)
                .out_arg(("realms", "a(sssy)")))

            .add_m(f.method("Start", (), Self::do_start)
                .in_arg(("name", "s")))

            .add_m(f.method("Stop", (), Self::do_stop)
                .in_arg(("name", "s")))

            .add_m(f.method("Restart", (), Self::do_restart)
                .in_arg(("name", "s")))

            .add_m(f.method("Terminal", (), Self::do_terminal)
                .in_arg(("name", "s")))

            .add_m(f.method("Run", (), Self::do_run)
                .in_arg(("name", "s"))
                .in_arg(("args", "as")))

            .add_m(f.method("RealmFromCitadelPid", (), Self::do_pid_to_realm)
                .in_arg(("pid", "u"))
                .out_arg(("realm", "s")))

            .add_m(f.method("RealmConfig", (), Self::do_get_realm_config)
                       .in_arg(("name", "s"))
                       .out_arg(("config", "a(ss)")))

            .add_m(f.method("ListRealmFS", (), Self::do_list_realmfs)
                .out_arg(("realmfs", "as")))

            .add_m(f.method("UpdateRealmFS", (), Self::do_update)
                .in_arg(("name", "s")))

            // Signals
            .add_s(f.signal("RealmStarted", ())
                .arg(("realm", "s")))
            .add_s(f.signal("RealmStopped", ())
                .arg(("realm", "s")))
            .add_s(f.signal("RealmNew", ())
                .arg(("realm", "s")))
            .add_s(f.signal("RealmRemoved", ())
                .arg(("realm","s")))
            .add_s(f.signal("RealmCurrent", ())
                .arg(("realm", "s")))
            .add_s(f.signal("ServiceStarted", ()));

        let obpath = f.object_path(OBJECT_PATH, ())
            .introspectable()
            .add(interface);

        f.tree(data).add(obpath)
    }

    fn do_list(m: &MethodInfo) -> MethodResult {
        let list = m.tree.get_data().realm_list();
        Ok(vec![m.msg.method_return().append1(list)])
    }

    fn do_set_current(m: &MethodInfo) -> MethodResult {
        let manager = m.tree.get_data().manager();
        let name = m.msg.read1()?;
        if let Some(realm) = manager.realm_by_name(name) {
            if let Err(err) = manager.set_current_realm(&realm) {
                warn!("set_current_realm({}) failed: {}", name, err);
            }
        }
        Ok(vec![m.msg.method_return()])
    }

    fn do_get_current(m: &MethodInfo) -> MethodResult {
        let manager = m.tree.get_data().manager();
        let ret = m.msg.method_return();
        let msg = match manager.current_realm() {
            Some(realm) => ret.append(realm.name()),
            None => ret.append(""),
        };
        Ok(vec![msg])
    }

    fn do_start(m: &MethodInfo) -> MethodResult {
        let name = m.msg.read1()?;
        let data = m.tree.get_data().clone();
        let realm = data.realm_by_name(name)?;
        thread::spawn(move || {
            if let Err(e) = data.manager().start_realm(&realm) {
                warn!("failed to start realm {}: {}", realm.name(), e);
            }
        });
        Ok(vec![m.msg.method_return()])
    }

    fn do_stop(m: &MethodInfo) -> MethodResult {
        let name = m.msg.read1()?;
        let data = m.tree.get_data().clone();
        let realm = data.realm_by_name(name)?;
        thread::spawn(move || {
            if let Err(e) = data.manager().stop_realm(&realm) {
                warn!("failed to stop realm {}: {}", realm.name(), e);
            }
        });
        Ok(vec![m.msg.method_return()])
    }

    fn do_restart(m: &MethodInfo) -> MethodResult {
        let name = m.msg.read1()?;
        let data = m.tree.get_data().clone();
        let realm = data.realm_by_name(name)?;
        thread::spawn(move || {
            if let Err(e) = data.manager().stop_realm(&realm) {
                warn!("failed to stop realm {}: {}", realm.name(), e);
            } else if let Err(e) = data.manager().start_realm(&realm) {
                warn!("failed to restart realm {}: {}", realm.name(), e);
            }
        });
        Ok(vec![m.msg.method_return()])
    }

    fn do_terminal(m: &MethodInfo) -> MethodResult {
        let name = m.msg.read1()?;
        let data = m.tree.get_data().clone();
        let realm = data.realm_by_name(name)?;
        thread::spawn(move || {
            if !realm.is_active() {
                if let Err(err) = data.manager().start_realm(&realm) {
                    warn!("failed to start realm {}: {}", realm.name(), err);
                    return;
                }
            }
            if let Err(err) = data.manager().launch_terminal(&realm) {
                warn!("error launching terminal for realm {}: {}", realm.name(), err);
            }
        });
        Ok(vec![m.msg.method_return()])
    }

    fn do_update(m: &MethodInfo) -> MethodResult {
        let name = m.msg.read1()?;
        let data = m.tree.get_data().clone();
        let realmfs = data.realmfs_by_name(name)?;

        let command = format!("{} {} update {}", SUDO_PATH, UPDATE_TOOL_PATH, realmfs.name());
        terminal::spawn_citadel_gnome_terminal(Some(command));

        Ok(vec![m.msg.method_return()])
    }

    fn do_run(m: &MethodInfo) -> MethodResult {
        let (name,args) = m.msg.read2::<&str, Vec<String>>()?;
        let data = m.tree.get_data().clone();
        let realm = data.realm_by_name(name)?;
        thread::spawn(move || {
            if !realm.is_active() {
                if let Err(err) = data.manager().start_realm(&realm) {
                    warn!("failed to start realm {}: {}", realm.name(), err);
                    return;
                }
            }
            if let Err(err) = data.manager().run_in_realm(&realm, &args, true) {
                warn!("error running {:?} in realm {}: {}", args, realm.name(), err);
            }
        });
        Ok(vec![m.msg.method_return()])
    }

    fn do_pid_to_realm(m: &MethodInfo) -> MethodResult {
        let pid = m.msg.read1::<u32>()?;
        let manager = m.tree.get_data().manager();
        let ret = m.msg.method_return();
        let msg = match manager.realm_by_pid(pid) {
            Some(realm) => ret.append(realm.name()),
            None => ret.append(""),
        };
        Ok(vec![msg])
    }

    fn do_get_realm_config(m: &MethodInfo) -> MethodResult {
        let name = m.msg.read1()?;
        let data = m.tree.get_data().clone();
        let config = data.realm_config(name)?;
        Ok(vec![m.msg.method_return().append1(config)])
    }

    fn do_list_realmfs(m: &MethodInfo) -> MethodResult {
        let list = m.tree.get_data().realmfs_list();
        Ok(vec![m.msg.method_return().append1(list)])
    }

    pub fn start(&self) -> Result<()> {
        let tree = self.build_tree();
        self.connection.register_name(BUS_NAME, NameFlag::ReplaceExisting as u32)?;
        tree.set_registered(&self.connection, true)?;
        self.connection.add_handler(tree);

        self.receive_signals_from(VPN_CONNECTION_INTERFACE)?;
        self.receive_signals_from(OBJECT_MANAGER_INTERFACE)?;

        self.manager.add_event_handler({
            let events = self.events.clone();
            move |ev| events.handle_event(ev)
        });

        if let Err(e) = self.manager.start_event_task() {
            warn!("error starting realm manager event task: {}", e);
        }

        self.send_service_started();

        loop {
            if let Some(msg) = self.connection.incoming(1000).next() {
                self.process_message(msg)?;
            }
        }
    }

    fn process_message(&self, _msg: Message) -> Result<()> {
        // add handlers for expected signals here
        Ok(())
    }

    fn receive_signals_from(&self, interface: &str) -> Result<()> {
        let rule = format!("type=signal,interface={}", interface);
        self.connection.add_match(rule.as_str())?;
        Ok(())
    }

    fn send_service_started(&self) {
        let signal = Self::create_signal("ServiceStarted");
        if self.connection.send(signal).is_err() {
            warn!("Failed to send ServiceStarted signal");
        }
    }

    fn create_signal(name: &str) -> Message {
        let path = dbus::Path::new(OBJECT_PATH).unwrap();
        let iface = dbus::Interface::new(INTERFACE_NAME).unwrap();
        let member = dbus::Member::new(name).unwrap();
        Message::signal(&path, &iface, &member)
    }

}

/// Wraps a connection instance and only expose the send() method.
/// Sending a message does not read or write any of the internal
/// Connection object state other than the native handle for the
/// connection. It should be safe to share this across threads as
/// internally libdbus uses a mutex to control concurrent access
/// to the dbus_connection_send() function.
#[derive(Clone)]
struct ConnectionSender(Arc<Connection>);

unsafe impl Send for ConnectionSender {}
unsafe impl Sync for ConnectionSender {}

impl ConnectionSender {
    fn new(connection: Arc<Connection>) -> Self {
        ConnectionSender(connection)
    }

    fn send(&self, msg: Message) -> Result<()> {
        self.0.send(msg)
            .map_err(|()| failure::err_msg("failed to send message"))?;
        Ok(())
    }
}

#[derive(Clone)]
struct EventHandler {
    sender: ConnectionSender,
}

impl EventHandler {
    fn new(conn: Arc<Connection>) -> EventHandler {
        EventHandler {
            sender: ConnectionSender::new(conn),
        }
    }

    fn handle_event(&self, ev: &RealmEvent) {
       match ev {
           RealmEvent::Started(realm) => self.on_started(realm),
           RealmEvent::Stopped(realm) => self.on_stopped(realm),
           RealmEvent::New(realm) => self.on_new(realm),
           RealmEvent::Removed(realm) => self.on_removed(realm),
           RealmEvent::Current(realm) => self.on_current(realm.as_ref()),
       }
    }

    fn on_started(&self, realm: &Realm) {
        self.send_realm_signal("RealmStarted", Some(realm));
    }

    fn on_stopped(&self, realm: &Realm) {
        self.send_realm_signal("RealmStopped", Some(realm));
    }

    fn on_new(&self, realm: &Realm) {
        self.send_realm_signal("RealmNew", Some(realm));
    }

    fn on_removed(&self, realm: &Realm) {
        self.send_realm_signal("RealmRemoved", Some(realm));
    }

    fn on_current(&self, realm: Option<&Realm>) {
        self.send_realm_signal("RealmCurrent", realm);
    }

    fn create_realm_signal(name: &str) -> Message {
        let path = dbus::Path::new(OBJECT_PATH).unwrap();
        let iface = dbus::Interface::new(INTERFACE_NAME).unwrap();
        let member = dbus::Member::new(name).unwrap();
        Message::signal(&path, &iface, &member)
    }

    fn send_realm_signal(&self, sig_name: &str, realm: Option<&Realm>) {
        let realm_name = match realm {
            Some(r) => r.name(),
            None => "",
        };

        let msg = Self::create_realm_signal(sig_name)
            .append1(realm_name);

        if let Err(e) = self.sender.send(msg) {
            warn!("Could not send signal '{}': {}", sig_name, e);
        }
    }
}

#[derive(Clone)]
struct TreeData {
    manager: Arc<RealmManager>,
}

impl TreeData {
    fn new(manager: Arc<RealmManager>) -> TreeData {
        TreeData {
            manager,
        }
    }

    fn manager(&self) -> &RealmManager {
        &self.manager
    }

    fn realm_by_name(&self, name: &str) -> result::Result<Realm, MethodErr> {
        if let Some(realm) = self.manager.realm_by_name(name) {
            Ok(realm)
        } else {
            result::Result::Err(MethodErr::failed(&format!("Cannot find realm {}", name)))
        }
    }

    fn realmfs_by_name(&self, name: &str) -> result::Result<RealmFS, MethodErr> {
        if let Some(realmfs) = self.manager.realmfs_by_name(name) {
            Ok(realmfs)
        } else {
            result::Result::Err(MethodErr::failed(&format!("Cannot find realmfs {}", name)))
        }
    }

    fn append_config_flag(list: &mut Vec<(String,String)>, val: bool, name: &str) {
        let valstr = if val { "true".to_string() } else { "false".to_string() };
        list.push((name.to_string(), valstr));
    }

    fn realm_config(&self, name: &str) -> result::Result<Vec<(String,String)>, MethodErr> {
        let realm = self.realm_by_name(name)?;
        let config = realm.config();
        let mut list = Vec::new();
        Self::append_config_flag(&mut list, config.gpu(), "use-gpu");
        Self::append_config_flag(&mut list, config.wayland(), "use-wayland");
        Self::append_config_flag(&mut list, config.x11(), "use-x11");
        Self::append_config_flag(&mut list, config.sound(), "use-sound");
        Self::append_config_flag(&mut list, config.shared_dir(), "use-shared-dir");
        Self::append_config_flag(&mut list, config.network(), "use-network");
        Self::append_config_flag(&mut list, config.kvm(), "use-kvm");
        Self::append_config_flag(&mut list, config.ephemeral_home(), "use-ephemeral-home");
        let overlay = match config.overlay() {
            OverlayType::None => "none",
            OverlayType::TmpFS => "tmpfs",
            OverlayType::Storage => "storage",
        };
        let scheme = match config.terminal_scheme() {
            Some(name) => name.to_string(),
            None => String::new(),
        };

        list.push(("realmfs".to_string(), config.realmfs().to_string()));
        list.push(("overlay".to_string(), overlay.to_string()));
        list.push(("terminal-scheme".to_string(), scheme));

        Ok(list)
    }

    fn realm_element(realm: &Realm) -> (String, String, String, u8) {
        let name = realm.name().to_owned();
        let desc = Self::realm_description(realm);
        let realmfs = realm.config().realmfs().to_owned();
        let status = Self::realm_status(realm);
        (name, desc, realmfs, status)
    }

    fn realm_list(&self) -> Vec<(String, String, String, u8)> {
        self.manager.realm_list()
            .iter()
            .map(Self::realm_element)
            .collect()
    }

    fn realm_description(realm: &Realm) -> String {
        match realm.notes() {
            Some(s) => s,
            None => String::new(),
        }
    }

    fn realm_status(realm: &Realm) -> u8 {
        let mut status = 0;
        if realm.is_active() {
            status |= STATUS_REALM_RUNNING;
        }
        if realm.is_current() {
            status |= STATUS_REALM_CURRENT;
        }
        if realm.is_system() {
            status |= STATUS_REALM_SYSTEM_REALM;
        }
        status
    }

    fn realmfs_list(&self) -> Vec<String> {
        self.manager.realmfs_list()
            .into_iter()
            .map(|fs| fs.name().to_owned())
            .collect()
    }
}
impl fmt::Debug for TreeData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<TreeData>")
    }
}

#[derive(Copy, Clone, Default, Debug)]
struct TData;

impl tree::DataType for TData {
    type Tree = TreeData;
    type ObjectPath = ();
    type Property = ();
    type Interface = ();
    type Method = ();
    type Signal = ();
}
