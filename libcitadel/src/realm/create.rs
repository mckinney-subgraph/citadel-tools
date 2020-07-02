use std::path::{PathBuf, Path};
use crate::{Realms, Result, util};
use std::fs;

/// Creation and removal of a Realm
pub struct RealmCreateDestroy {
    name: String,
}

impl RealmCreateDestroy {

    pub fn new(name: &str) -> Self {
        let name = name.to_string();
        RealmCreateDestroy { name }
    }

    fn tmpdir() -> PathBuf {
        Path::new(Realms::BASE_PATH).join(".tmp")
    }

    pub fn temp_basepath(&self) -> PathBuf {
        Self::tmpdir().join(self.dirname())
    }

    pub fn basepath(&self) -> PathBuf {
        Path::new(Realms::BASE_PATH)
            .join(self.dirname())
    }

    fn dirname(&self) -> String {
        format!("realm-{}", self.name)
    }

    /// Create a new realm with the name `self.name`
    pub fn create(&self) -> Result<()> {
        if self.basepath().exists() {
            bail!("realm directory {} already exists", self.basepath().display());
        }

        if let Err(e) = self.create_realm_directory() {
            let tmpdir = self.temp_basepath();
            if tmpdir.exists() {
                let _ = fs::remove_dir_all(tmpdir);
            }
            return Err(e);
        }
        Ok(())
    }

    fn create_realm_directory(&self) -> Result<()> {
        self.create_home()?;
        self.move_from_temp()
    }

    fn create_home(&self) -> Result<()> {
        let home = self.temp_basepath().join("home");

        util::create_dir(&home)?;
        util::chown(&home, 1000, 1000)?;

        let skel = Path::new(Realms::BASE_PATH).join("skel");

        if skel.exists() {
            info!("Populating realm home directory with files from {}", skel.display());
            util::copy_tree(&skel, &home)
                .map_err(context!("failed to copy tree of files from {:?} to {:?}", skel, home))?;
        }
        Ok(())
    }

    fn move_from_temp(&self) -> Result<()> {
        let from = self.temp_basepath();
        let to = self.basepath();
        if to.exists() {
            bail!("Cannot move temporary directory {} to {} because the target already exists", from.display(), to.display());
        }
        util::rename(&from, &to)
    }

    fn move_to_temp(&self) -> Result<()> {
        let from = self.basepath();
        let to = self.temp_basepath();
        if to.exists() {
            bail!("Cannot move realm directory {} to {} because the target already exists", from.display(), to.display());
        }

        let tmpdir = Self::tmpdir();
        util::create_dir(&tmpdir)?;
        util::rename(&from, &to)
    }

    pub fn delete_realm(&self, save_home: bool) -> Result<()> {

        self.move_to_temp()?;
        if save_home {
            self.save_home_for_delete()?;
        }

        let realmdir = self.temp_basepath();
        info!("removing realm directory {:?}", realmdir);
        fs::remove_dir_all(&realmdir)
            .map_err(context!("error removing realm directory {:?}", realmdir))
    }

    fn save_home_for_delete(&self) -> Result<()> {
        util::create_dir("/realms/removed")?;

        let target = self.home_save_directory();
        let home = self.temp_basepath().join("home");

        util::rename(&home, &target)?;
        info!("home directory been moved to {}, delete it at your leisure", target.display());
        Ok(())
    }

    fn home_save_directory(&self) -> PathBuf {
        let mut n = 1;
        let mut save_dir= PathBuf::from(&format!("/realms/removed/home-{}", self.name));
        while save_dir.exists() {
            save_dir.set_extension(n.to_string());
            n += 1;
        }
        save_dir
    }

}