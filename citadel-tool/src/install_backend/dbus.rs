use std::sync::Arc;
use std::collections::HashMap;
use std::time::Duration;
use std::sync::mpsc;
use std::sync::mpsc::{Sender};

use dbus::tree::{self, Factory, MTFn, MethodResult, Tree};
use dbus::{Message};
use dbus::blocking::LocalConnection;
use libcitadel::{Result};
// Use local version of disk.rs since we added some methods
use crate::install_backend::disk::*;
use crate::install::installer::*;
use std::fmt;

type MethodInfo<'a> = tree::MethodInfo<'a, MTFn<TData>, TData>;


const OBJECT_PATH: &str = "/com/subgraph/installer";
const INTERFACE_NAME: &str = "com.subgraph.installer.Manager";
const BUS_NAME: &str = "com.subgraph.installer";

pub enum Msg {
    RunInstall(String, String, String),
    LvmSetup(String),
    LuksSetup(String),
    BootSetup(String),
    StorageCreated(String),
    RootfsInstalled(String),
    InstallCompleted,
    InstallFailed(String)
}

pub struct DbusServer {
    connection: Arc<LocalConnection>,
    //events: EventHandler,
}

impl DbusServer {

    pub fn connect() -> Result<DbusServer> {
        let connection = LocalConnection::new_system()
            .map_err(|e| format_err!("Failed to connect to DBUS system bus: {}", e))?;
        let connection = Arc::new(connection);
        //let events = EventHandler::new(connection.clone());
        let server = DbusServer { connection };
        Ok(server)

    }

    fn build_tree(&self, sender: mpsc::Sender<Msg>) -> Tree<MTFn<TData>, TData> {
        let f = Factory::new_fn::<TData>();
        let data = TreeData::new();
        let interface = f.interface(INTERFACE_NAME, ())
            // Methods
            .add_m(f.method("GetDisks", (), Self::do_get_disks)
                .in_arg(("name", "a{sas}")))
            
            .add_m(f.method("RunInstall", (),move |m| {

                let (device, citadel_passphrase, luks_passphrase): (String, String, String) = m.msg.read3()?;
                println!("Device: {} Citadel Passphrase: {} Luks Passphrase: {}", device, citadel_passphrase, luks_passphrase);
                let _ = sender.send(Msg::RunInstall(device, citadel_passphrase, luks_passphrase));
                Ok(vec![m.msg.method_return().append1(true)])
            })
                .in_arg(("device", "s")).in_arg(("citadel_passphrase", "s")).in_arg(("luks_passphrase", "s")))
            .add_s(f.signal("RunInstallStarted", ()))
            .add_s(f.signal("InstallCompleted", ()))
            .add_s(f.signal("CitadelPasswordSet", ()));
        let obpath = f.object_path(OBJECT_PATH, ())
            .introspectable()
            .add(interface);

        f.tree(data).add(obpath)
    }


    fn do_get_disks(m: &MethodInfo) -> MethodResult {
        let list = m.tree.get_data().disks();
        Ok(vec![m.msg.method_return().append1(list)])
    }

    fn run_install(path: String, citadel_passphrase: String, luks_passphrase: String, sender: Sender<Msg>) -> Result<()> {
        let mut install = Installer::new(path, &citadel_passphrase, &luks_passphrase);
        install.set_install_syslinux(true);
        install.verify()?;
        install.partition_disk()?;
        install.setup_luks()?;
        let _ = sender.send(Msg::LuksSetup("+ Setup LUKS disk encryption password successfully\n".to_string()));
        install.setup_lvm()?;
        let _ = sender.send(Msg::LvmSetup("+ Setup LVM volumes successfully\n".to_string()));
        install.setup_boot()?;
        let _ = sender.send(Msg::BootSetup("+ Setup /boot partition successfully\n".to_string()));
        install.create_storage()?;
        let _ = sender.send(Msg::StorageCreated("+ Setup /storage partition successfully\n".to_string()));
        install.install_rootfs_partitions()?;
        let _ = sender.send(Msg::RootfsInstalled("+ Installed rootfs partitions successfully\n".to_string()));
        install.finish_install()?;
        Ok(())
    }

    /*fn process_message(&self, _msg: Message) -> Result<()> {
        // add handlers for expected signals here
        Ok(())
    }*/

    fn send_service_started(&self) {
        let signal = Self::create_signal("ServiceStarted");
        if self.connection.channel().send(signal).is_err() {
            warn!("Failed to send ServiceStarted signal");
        }
    }
    
    fn send_install_completed(&self) {
        let signal = Self::create_signal("InstallCompleted");
        if self.connection.channel().send(signal).is_err() {
            warn!("Failed to send InstallCompleted signal");
        }
    }

    fn send_lvm_setup(&self, text: String) {
        let signal = Self::create_signal_with_text("LvmSetup", text);
        if self.connection.channel().send(signal).is_err() {
            warn!("Failed to send LvmSetup signal");
        }
    }

    fn send_luks_setup(&self, text: String) {
        let signal = Self::create_signal_with_text("LuksSetup", text);
        if self.connection.channel().send(signal).is_err() {
            warn!("Failed to send LuksSetup signal");
        }
    }

    fn send_boot_setup(&self, text: String) {
        let signal = Self::create_signal_with_text("BootSetup", text);
        if self.connection.channel().send(signal).is_err() {
            warn!("Failed to send BootSetup signal");
        }
    }

    fn send_storage_created(&self, text: String) {
        let signal = Self::create_signal_with_text("StorageCreated", text);
        if self.connection.channel().send(signal).is_err() {
            warn!("Failed to send StorageCreated signal");
        }
    }

    fn send_rootfs_installed(&self, text: String) {
        let signal = Self::create_signal_with_text("RootfsInstalled", text);
        if self.connection.channel().send(signal).is_err() {
            warn!("Failed to send StorageCreated signal");
        }
    }

    fn send_install_failed(&self, error: String) {
        let signal = Self::create_signal_with_text("InstallFailed", error);
        if self.connection.channel().send(signal).is_err() {
            warn!("Failed to send StorageCreated signal");
        }
    }

    fn create_signal(name: &str) -> Message {
        let path = dbus::Path::new(OBJECT_PATH).unwrap();
        let iface = dbus::strings::Interface::new(INTERFACE_NAME).unwrap();
        let member = dbus::strings::Member::new(name).unwrap();
        Message::signal(&path, &iface, &member)
    }

    fn create_signal_with_text(name: &str, text: String) -> Message {
        let path = dbus::Path::new(OBJECT_PATH).unwrap();
        let iface = dbus::strings::Interface::new(INTERFACE_NAME).unwrap();
        let member = dbus::strings::Member::new(name).unwrap();
        Message::signal(&path, &iface, &member).append1(text)
    }

    pub fn start(&self) -> Result<()> {
        let (sender, receiver) = mpsc::channel::<Msg>(); 
        let sender_clone = sender.clone();
        let tree = self.build_tree(sender);
        if let Err(_err) = self.connection.request_name(BUS_NAME, false, true, false) {
            bail!("Failed to request name");
        }

        tree.start_receive(self.connection.as_ref());

        self.send_service_started();
        loop {
            self.connection
                .process(Duration::from_millis(1000))
                .map_err(context!("Error handling dbus messages"))?;

            if let Ok(msg) = receiver.try_recv() {
                match msg {
                    Msg::RunInstall(device, citadel_passphrase, luks_passphrase) => {
                        let install_sender = sender_clone.clone();
                        // TODO: Implement more stages
                        match Self::run_install(device, citadel_passphrase, luks_passphrase, install_sender) {
                            Ok(_) => {
                                println!("Install completed"); 
                                let _ = sender_clone.send(Msg::InstallCompleted);
                            },
                            Err(err) => {
                                println!("Install error: {}", err);
                                let _ = sender_clone.send(Msg::InstallFailed(err.to_string()));
                            }
                        }
                    },
                    Msg::LvmSetup(text) => {
                        self.send_lvm_setup(text);
                    },
                    Msg::LuksSetup(text) => {
                        self.send_luks_setup(text);
                    },
                    Msg::BootSetup(text) => {
                        self.send_boot_setup(text);
                    },
                    Msg::StorageCreated(text) => {
                        self.send_storage_created(text);
                    },
                    Msg::RootfsInstalled(text) => {
                        self.send_rootfs_installed(text);
                    },
                    Msg::InstallCompleted => {
                        self.send_install_completed();
                    },
                    Msg::InstallFailed(text) => {
                        self.send_install_failed(text);
                    }
                }
            }
        }
    }

}

#[derive(Clone)]
struct TreeData {
}

impl TreeData {
    fn new() -> TreeData {
        TreeData {}
    }


    fn disks(&self) -> HashMap<String, Vec<String>> {
        let disks = Disk::probe_all().unwrap();
         
        let mut disk_map = HashMap::new();
        for d in disks {
            let mut fields = vec![];
            fields.push(d.model().to_string());
            fields.push(d.size_str().to_string());
            fields.push(d.removable().to_string());
            disk_map.insert(d.path().to_string_lossy().to_string(), fields);
        }
        disk_map
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
