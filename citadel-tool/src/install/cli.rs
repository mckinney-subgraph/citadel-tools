use std::io::{self,Write};
use std::path::Path;
use libcitadel::Result;
use super::disk::Disk;
use rpassword;
use crate::install::installer::Installer;

const CITADEL_PASSPHRASE_PROMPT: &str = "Enter a password for the Citadel user (or 'q' to quit)";
const LUKS_PASSPHRASE_PROMPT: &str = "Enter a disk encryption passphrase (or 'q' to quit";

pub fn run_cli_install() -> Result<bool> {
    let disk = match choose_disk()? {
        Some(disk) => disk,
        None => return Ok(false),
    };

    display_disk(&disk);

    let citadel_passphrase = match read_passphrase(CITADEL_PASSPHRASE_PROMPT).map_err(context!("error reading citadel user passphrase"))? {
        Some(citadel_passphrase) => citadel_passphrase,
        None => return Ok(false),
    };

    let passphrase = match read_passphrase(LUKS_PASSPHRASE_PROMPT).map_err(context!("error reading luks passphrase"))? {
        Some(passphrase) => passphrase,
        None => return Ok(false),
    };

    if !confirm_install(&disk)? {
        return Ok(false);
    }
    run_install(disk, citadel_passphrase, passphrase)?;
    Ok(true)
}

pub fn run_cli_install_with<P: AsRef<Path>>(target: P) -> Result<bool> {
    let disk = find_disk_by_path(target.as_ref())?;
    display_disk(&disk);

    let citadel_passphrase = match read_passphrase(CITADEL_PASSPHRASE_PROMPT).map_err(context!("error reading citadel user passphrase"))? {
        Some(citadel_passphrase) => citadel_passphrase,
        None => return Ok(false),
    };

    let passphrase = match read_passphrase(LUKS_PASSPHRASE_PROMPT).map_err(context!("error reading luks passphrase"))? {
        Some(passphrase) => passphrase,
        None => return Ok(false),
    };

    if !confirm_install(&disk)? {
        return Ok(false);
    }

    run_install(disk, citadel_passphrase, passphrase)?;
    Ok(true)
}

fn run_install(disk: Disk, citadel_passphrase: String, passphrase: String) -> Result<()> {
    let mut install = Installer::new(disk.path(), &citadel_passphrase, &passphrase);
    install.set_install_syslinux(true);
    install.verify()?;
    install.run()
}

fn display_disk(disk: &Disk) {
    println!();
    println!("  Device: {}", disk.path().display());
    println!("    Size: {}", disk.size_str());
    println!("   Model: {}", disk.model());
    println!();
}

fn find_disk_by_path(path: &Path) -> Result<Disk> {
    if !path.exists() {
        bail!("Target disk path {} does not exist", path.display());
    }
    for disk in Disk::probe_all()? {
        if disk.path() == path {
            return Ok(disk.clone());
        }
    }
    bail!("installation target {} is not a valid disk", path.display())
}

fn choose_disk() -> Result<Option<Disk>> {
    let disks = Disk::probe_all()?;
    if disks.is_empty() {
        bail!("no disks found.");
    }

    loop {
        prompt_choose_disk(&disks);
        let line = read_line()?;
        if line == "q" || line == "Q" {
            return Ok(None);
        }
        if let Ok(n) = line.parse::<usize>() {
            if n > 0 && n <= disks.len() {
                return Ok(Some(disks[n-1].clone()));
            }
        }
    }
}

fn prompt_choose_disk(disks: &[Disk]) {
    println!("Available disks:\n");
    for (idx,disk) in disks.iter().enumerate() {
        println!("  [{}]: {} Size: {} Model: {}", idx + 1, disk.path().display(), disk.size_str(), disk.model());
    }
    print!("\nChoose a disk to install to (q to quit): ");
    let _ = io::stdout().flush();
}

fn read_line() -> Result<String> {
    let mut input = String::new();
    io::stdin().read_line(&mut input)
        .map_err(context!("error reading line from stdin"))?;
    if input.ends_with('\n') {
        input.pop();
    }
    Ok(input)
}

fn read_passphrase(prompt: &str) -> io::Result<Option<String>> {
    loop {
        println!("{}", prompt);
        println!();
        let passphrase = rpassword::read_password_from_tty(Some("  Passphrase : "))?;
        if passphrase.is_empty() {
            println!("Passphrase cannot be empty");
            continue;
        }
        if passphrase == "q" || passphrase == "Q" {
            return Ok(None);
        }
        let confirm    = rpassword::read_password_from_tty(Some("  Confirm    : "))?;
        if confirm == "q" || confirm == "Q" {
            return Ok(None);
        }
        println!();
        if passphrase == confirm {
            return Ok(Some(passphrase));
        }
        println!("Passphrases do not match");
        println!();
    }
}

fn confirm_install(disk: &Disk) -> Result<bool> {
    println!("Are you sure you want to completely erase this this device?");
    println!();
    println!("  Device: {}", disk.path().display());
    println!("    Size: {}", disk.size_str());
    println!("   Model: {}", disk.model());
    println!();
    print!("Type YES (uppercase) to continue with install: ");
    let _ = io::stdout().flush();
    let answer = read_line()?;
    Ok(answer == "YES")
}

