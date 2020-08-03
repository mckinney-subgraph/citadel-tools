#[macro_use] extern crate libcitadel;
use libcitadel::{RealmManager, Result, Logger, LogLevel};

mod dbus;
mod devices;

fn main() {
    if let Err(e) = run_dbus_server() {
        warn!("Error: {}", e);
    }
}

fn run_dbus_server() -> Result<()> {
    Logger::set_log_level(LogLevel::Verbose);
    let manager = RealmManager::load()?;
    let server = dbus::DbusServer::connect(manager)?;
    server.start()?;
    Ok(())
}
