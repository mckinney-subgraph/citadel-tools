
use crate::Result;
use std::process::Command;
use std::os::unix::process::CommandExt;
use crate::util::is_euid_root;
use std::thread;

const GNOME_TERMINAL_PATH: &str = "/usr/bin/gnome-terminal";

const TERMINAL_ENVIRONMENT: &[(&str, &str)] = &[
    ("DBUS_SESSION_BUS_ADDRESS", "unix:path=/run/user/1000/bus"),
    ("XDG_RUNTIME_DIR", "/run/user/1000"),
    ("XDG_SESSION_TYPE", "wayland"),
    ("GNOME_DESKTOP_SESSION_ID", "this-is-deprecated"),
    ("NO_AT_BRIDGE", "1")
];

#[allow(dead_code)]
pub struct GnomeTerminal {
    command: Command,
    args: Option<Vec<String>>,
}

#[allow(dead_code)]
impl GnomeTerminal {
    fn create_command() -> Command {
        let mut cmd = Command::new(GNOME_TERMINAL_PATH);
        if is_euid_root() {
            cmd.uid(1000);
            cmd.gid(1000);
        }
        cmd.arg("--quiet");
        // block until terminal window is closed
        cmd.arg("--wait");
        cmd
    }
    pub fn new() -> Self {
        GnomeTerminal {
            command: Self::create_command(),
            args: None,
        }
    }



}

fn build_open_terminal_command<S: AsRef<str>>(command: Option<S>) -> Command {
    let mut cmd = Command::new(GNOME_TERMINAL_PATH);
    cmd.envs(TERMINAL_ENVIRONMENT.to_vec());
    if is_euid_root() {
        cmd.uid(1000);
        cmd.gid(1000);
    }

    cmd.arg("--quiet");
    // block until terminal window is closed
    cmd.arg("--wait");

    if let Some(args) = command {
        cmd.arg("--");
        cmd.args(args.as_ref().split_whitespace());
    }
    cmd

}

pub fn spawn_citadel_gnome_terminal<S>(command: Option<S>)
  where S: 'static + Send + AsRef<str>
{
    thread::spawn(move || {
        if let Err(err) = open_citadel_gnome_terminal(command) {
            warn!("Failed to launch {}: {}", GNOME_TERMINAL_PATH, err);
        }
    });
}

pub fn open_citadel_gnome_terminal<S: AsRef<str>>(command: Option<S>) -> Result<()>
{
    let mut cmd = build_open_terminal_command(command);
    let status = cmd.status().map_err(context!("error running gnome-terminal"))?;
    info!("Gnome terminal exited with: {}", status);
    Ok(())
}