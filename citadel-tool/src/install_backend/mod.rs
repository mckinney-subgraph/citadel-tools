use libcitadel::Result;
use std::process::exit;

mod disk;
mod dbus;
use libcitadel::CommandLine;

pub fn main() {
    if CommandLine::live_mode() || CommandLine::install_mode() {
        if let Err(e) = run_dbus_server() {
            warn!("Error: {}", e);
        }
    } else {
        println!("Citadel installer backend will only run in install or live mode");
        exit(1);
    }
}

fn run_dbus_server() -> Result<()> {
    let server = dbus::DbusServer::connect()?;
    server.start()?;
    Ok(())
}

