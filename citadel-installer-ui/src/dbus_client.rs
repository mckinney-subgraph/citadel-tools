use dbus::arg;

#[derive(Debug)]
pub struct ComSubgraphInstallerManagerInstallCompleted {
}

impl arg::AppendAll for ComSubgraphInstallerManagerInstallCompleted {
    fn append(&self, _: &mut arg::IterAppend) {
    }
}

impl arg::ReadAll for ComSubgraphInstallerManagerInstallCompleted {
    fn read(_i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
        Ok(ComSubgraphInstallerManagerInstallCompleted {})
    }
}

impl dbus::message::SignalArgs for ComSubgraphInstallerManagerInstallCompleted {
    const NAME: &'static str = "InstallCompleted";
    const INTERFACE: &'static str = "com.subgraph.installer.Manager";
}

#[derive(Debug)]
pub struct ComSubgraphInstallerManagerRunInstallStarted {
    pub text: String,
}

impl arg::AppendAll for ComSubgraphInstallerManagerRunInstallStarted {
    fn append(&self, _: &mut arg::IterAppend) {
    }
}

impl arg::ReadAll for ComSubgraphInstallerManagerRunInstallStarted {
    fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
        Ok(ComSubgraphInstallerManagerRunInstallStarted {
            text: i.read()?,
        })
    }
}

impl dbus::message::SignalArgs for ComSubgraphInstallerManagerRunInstallStarted {
    const NAME: &'static str = "RunInstallStarted";
    const INTERFACE: &'static str = "com.subgraph.installer.Manager";
}

#[derive(Debug)]
pub struct ComSubgraphInstallerManagerDiskPartitioned {
    pub text: String,
}

impl arg::AppendAll for ComSubgraphInstallerManagerDiskPartitioned {
    fn append(&self, _: &mut arg::IterAppend) {
    }
}

impl arg::ReadAll for ComSubgraphInstallerManagerDiskPartitioned {
    fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
        Ok(ComSubgraphInstallerManagerDiskPartitioned {
            text: i.read()?
            //sender,
        })
    }
}

impl dbus::message::SignalArgs for ComSubgraphInstallerManagerDiskPartitioned {
    const NAME: &'static str = "DiskPartitioned";
    const INTERFACE: &'static str = "com.subgraph.installer.Manager";
}

#[derive(Debug)]
pub struct ComSubgraphInstallerManagerLvmSetup {
    pub text: String,
}

impl arg::AppendAll for ComSubgraphInstallerManagerLvmSetup {
    fn append(&self, _: &mut arg::IterAppend) {
    }
}

impl arg::ReadAll for ComSubgraphInstallerManagerLvmSetup {
    fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
        Ok(ComSubgraphInstallerManagerLvmSetup {
            text: i.read()?
        })
    }
}

impl dbus::message::SignalArgs for ComSubgraphInstallerManagerLvmSetup {
    const NAME: &'static str = "LvmSetup";
    const INTERFACE: &'static str = "com.subgraph.installer.Manager";
}

#[derive(Debug)]
pub struct ComSubgraphInstallerManagerLuksSetup {
    pub text: String,
}

impl arg::AppendAll for ComSubgraphInstallerManagerLuksSetup {
    fn append(&self, _: &mut arg::IterAppend) {
    }
}

impl arg::ReadAll for ComSubgraphInstallerManagerLuksSetup {
    fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
        Ok(ComSubgraphInstallerManagerLuksSetup {
            text: i.read()?
        })
    }
}

impl dbus::message::SignalArgs for ComSubgraphInstallerManagerLuksSetup {
    const NAME: &'static str = "LuksSetup";
    const INTERFACE: &'static str = "com.subgraph.installer.Manager";
}

#[derive(Debug)]
pub struct ComSubgraphInstallerManagerBootSetup {
    pub text: String,
}

impl arg::AppendAll for ComSubgraphInstallerManagerBootSetup {
    fn append(&self, _: &mut arg::IterAppend) {
    }
}

impl arg::ReadAll for ComSubgraphInstallerManagerBootSetup {
    fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
        Ok(ComSubgraphInstallerManagerBootSetup {
            text: i.read()?
        })
    }
}

impl dbus::message::SignalArgs for ComSubgraphInstallerManagerBootSetup {
    const NAME: &'static str = "BootSetup";
    const INTERFACE: &'static str = "com.subgraph.installer.Manager";
}

#[derive(Debug)]
pub struct ComSubgraphInstallerManagerStorageCreated {
    pub text: String,
}

impl arg::AppendAll for ComSubgraphInstallerManagerStorageCreated {
    fn append(&self, _: &mut arg::IterAppend) {
    }
}

impl arg::ReadAll for ComSubgraphInstallerManagerStorageCreated {
    fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
        Ok(ComSubgraphInstallerManagerStorageCreated {
            //sender,
            text: i.read()?
        })
    }
}

impl dbus::message::SignalArgs for ComSubgraphInstallerManagerStorageCreated {
    const NAME: &'static str = "StorageCreated";
    const INTERFACE: &'static str = "com.subgraph.installer.Manager";
}

#[derive(Debug)]
pub struct ComSubgraphInstallerManagerRootfsInstalled {
    pub text: String,
}

impl arg::AppendAll for ComSubgraphInstallerManagerRootfsInstalled {
    fn append(&self, _: &mut arg::IterAppend) {
    }
}

impl arg::ReadAll for ComSubgraphInstallerManagerRootfsInstalled {
    fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
        Ok(ComSubgraphInstallerManagerRootfsInstalled {
            text: i.read()?
        })
    }
}

impl dbus::message::SignalArgs for ComSubgraphInstallerManagerRootfsInstalled {
    const NAME: &'static str = "RootfsInstalled";
    const INTERFACE: &'static str = "com.subgraph.installer.Manager";
}

#[derive(Debug)]
pub struct ComSubgraphInstallerManagerInstallFailed {
    pub text: String,
}

impl arg::AppendAll for ComSubgraphInstallerManagerInstallFailed {
    fn append(&self, _: &mut arg::IterAppend) {
    }
}

impl arg::ReadAll for ComSubgraphInstallerManagerInstallFailed {
    fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
        Ok(ComSubgraphInstallerManagerInstallFailed {
            text: i.read()?,
        })
    }
}

impl dbus::message::SignalArgs for ComSubgraphInstallerManagerInstallFailed {
    const NAME: &'static str = "InstallFailed";
    const INTERFACE: &'static str = "com.subgraph.installer.Manager";
}
