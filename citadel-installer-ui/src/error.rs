
use std::result;

use dbus;
pub type Result<T> =  result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Dbus(dbus::Error),
    Builder(String),
}
