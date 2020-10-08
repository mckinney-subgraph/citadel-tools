# Citadel Installer UI design

The installer is required to run in Wayland but also perform privileged
operations. This necessitated splitting the installer into the following 
pieces:

- A user interface that can be run by a non-privileged user in their Wayland
session
- A back-end server that runs in the background to perform the privileged
operations on behalf of the user

The user interface communicates with the back-end over DBUS. There are a simple
set of messages/signals to initiate the install process and provide updates to
the interface about the success/failure of each install stage.

Both the user interface can only be run in install/live mode. The user 
interface will start automatically when the computer is booted in install/live 
mode, however, the user can close the interface and test out the system in 
live mode to determine if it is compatible with their hardware, if they want to 
actually perform an install, etc. If the user decides to install the system, 
they can simply re-open the user interface while still in live mode.


