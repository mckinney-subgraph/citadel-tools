use std::os::unix::io::RawFd;
use std::thread;

use glib::Continue;
use nix::unistd::close;
use nix::errno::Errno;
use nix::sys::socket::{
    socket,listen,bind,connect,accept,AddressFamily,SockType,SockFlag,SockAddr,UnixAddr
};

use crate::{Error,Result};

static SOCKET_NAME: &[u8] = b"citadel-realms-ui";

enum BindResult {
    BindOk,
    BindFailed(Error),
    BindAddrInUse,
}

///
/// Determine if another instance is already running and if so signal it to quit.
///
/// This window is launched from a GNOME shortcut key that is meant to 'toggle' the
/// window so that if the shortcut key is used while the window is already open the
/// running instance will close.
///
/// This class will attempt to create a Unix domain stream socket in the abstract
/// namespace bound to the fixed name `SOCKET_NAME`. If no other instance is running
/// then this name will be available and the bind will succeed. In this case a thread
/// is spawned to listen for connections to the socket and the process will exit
/// the main GTK loop by calling `gtk::main_quit()` upon a connection to the listening
/// socket.
///
/// If the bind fails because the socket name is already in use, then another instance is
/// running. A connection is then made to the socket to signal the running instance to exit.
///
pub struct InstanceTracker {
    fd: RawFd,
}

impl InstanceTracker {
    pub fn create() -> Result<Self> {
        let fd = socket(AddressFamily::Unix, SockType::Stream, SockFlag::empty(), None)
            .map_err(Error::Nix)?;
        Ok(InstanceTracker { fd } )
    }

    fn addr() -> SockAddr {
        SockAddr::Unix(UnixAddr::new_abstract(SOCKET_NAME)
                       .expect("UnixAddr::new_abstract()"))
    }

    fn try_bind(&self) -> BindResult {
        let addr = Self::addr();
        match bind(self.fd, &addr) {
            Err(nix::Error::Sys(Errno::EADDRINUSE)) => BindResult::BindAddrInUse,
            Err(err) => BindResult::BindFailed(Error::Nix(err)),
            Ok(()) => BindResult::BindOk,
        }
    }

    fn connect(&self) -> bool {
        let addr = Self::addr();
        if let Err(err) = connect(self.fd, &addr) {
            println!("Failed to connect to instance socket: {}", err);
            return false;
        }
        if let Err(err) = close(self.fd) {
            println!("error closing socket: {}", err);
        }
        true
    }

    fn spawn_reader(&self) {
        thread::spawn({
            let fd = self.fd;
            move || {
                let _ = listen(fd, 1);
                let _ = accept(fd);
                glib::idle_add(|| {
                    gtk::main_quit();
                    Continue(false)
                });
            }
        });
    }

    pub fn bind(&self, toggle: bool) -> bool {
        match self.try_bind() {
            BindResult::BindAddrInUse => {
                if toggle {
                    self.connect();
                }
                false
            },

            BindResult::BindOk => {
                self.spawn_reader();
                true
            }
            BindResult::BindFailed(err) => {
                println!("error binding: {:?}", err);
                false
            }
        }
    }
}


