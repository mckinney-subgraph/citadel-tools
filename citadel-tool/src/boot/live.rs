use std::path::Path;
use std::ffi::OsStr;
use std::fs;
use std::thread::{self,JoinHandle};
use std::time::{self,Instant};

use libcitadel::{Result, UtsName, util};
use libcitadel::ResourceImage;

use crate::boot::disks;
use crate::boot::rootfs::setup_rootfs_resource;
use crate::install::installer::Installer;

const IMAGE_DIRECTORY: &str = "/run/citadel/images";

pub fn live_rootfs() -> Result<()> {
    copy_artifacts()?;
    let rootfs = find_rootfs_image()?;
    setup_rootfs_resource(&rootfs)
}

pub fn live_setup() -> Result<()> {
    decompress_images()?;
    info!("Starting live setup");
    let live = Installer::new_livesetup();
    live.run()
}

fn copy_artifacts() -> Result<()> {
    for _ in 0..3 {
        if try_copy_artifacts()? {
            //decompress_images()?;
            return Ok(())
        }
        // Try again after waiting for more devices to be discovered
        info!("Failed to find partition with images, trying again in 2 seconds");
        thread::sleep(time::Duration::from_secs(2));
    }
    bail!("could not find partition containing resource images")

}

fn try_copy_artifacts() -> Result<bool> {
    let rootfs_image = Path::new("/boot/images/citadel-rootfs.img");
    // Already mounted?
    if rootfs_image.exists() {
        deploy_artifacts()?;
        return Ok(true);
    }
    for part in disks::DiskPartition::boot_partitions(false)? {
        part.mount("/boot")?;

        if rootfs_image.exists() {
            deploy_artifacts()?;
            part.umount()?;
            return Ok(true);
        }
        part.umount()?;
    }
    Ok(false)
}

fn kernel_version() -> String {
    let utsname = UtsName::uname();
    let v = utsname.release().split('-').collect::<Vec<_>>();
    v[0].to_string()
}

fn deploy_artifacts() -> Result<()> {
    let run_images = Path::new(IMAGE_DIRECTORY);
    if !run_images.exists() {
        util::create_dir(run_images)?;
        cmd!("/bin/mount", "-t tmpfs -o size=4g images /run/citadel/images")?;
    }

    util::read_directory("/boot/images", |dent| {
        println!("Copying {:?} from /boot/images to /run/citadel/images", dent.file_name());
        util::copy_file(dent.path(), run_images.join(dent.file_name()))
    })?;

    let kv = kernel_version();
    println!("Copying bzImage-{} to /run/citadel/images", kv);
    let from = format!("/boot/bzImage-{}", kv);
    let to = format!("/run/citadel/images/bzImage-{}", kv);
    util::copy_file(&from, &to)?;

    println!("Copying bootx64.efi to /run/citadel/images");
    util::copy_file("/boot/EFI/BOOT/bootx64.efi", "/run/citadel/images/bootx64.efi")?;

    deploy_syslinux_artifacts()?;

    Ok(())
}

fn deploy_syslinux_artifacts() -> Result<()> {
    let boot_syslinux = Path::new("/boot/syslinux");

    if !boot_syslinux.exists() {
        println!("Not copying syslinux components because /boot/syslinux does not exist");
        return Ok(());
    }

    println!("Copying contents of /boot/syslinux to /run/citadel/images/syslinux");

    let run_images_syslinux = Path::new("/run/citadel/images/syslinux");
    util::create_dir(run_images_syslinux)?;

    util::read_directory(boot_syslinux, |dent| {
        if let Some(ext) = dent.path().extension() {
            if ext == "c32" || ext == "bin" {
                util::copy_file(dent.path(), run_images_syslinux.join(dent.file_name()))?;
            }
        }
        Ok(())
    })
}

fn find_rootfs_image() -> Result<ResourceImage> {
    let entries = fs::read_dir(IMAGE_DIRECTORY)
        .map_err(context!("error reading directory {}", IMAGE_DIRECTORY))?;
    for entry in entries {
        let entry = entry.map_err(context!("error reading directory entry"))?;
        if entry.path().extension() == Some(OsStr::new("img")) {
            if let Ok(image) = ResourceImage::from_path(&entry.path()) {
                if image.metainfo().image_type() == "rootfs" {
                    return Ok(image)
                }
            }
        }
    }
    bail!("unable to find rootfs resource image in {}", IMAGE_DIRECTORY)
}

fn decompress_images() -> Result<()> {
    info!("Decompressing images");
    let mut threads = Vec::new();
    util::read_directory("/run/citadel/images", |dent| {
        if dent.path().extension() == Some(OsStr::new("img")) {
            if let Ok(image) = ResourceImage::from_path(&dent.path()) {
                if image.is_compressed() {
                    threads.push(decompress_one_image(image));
                }
            }
        }
        Ok(())
    })?;

    for t in threads {
        t.join().unwrap()?;
    }
    info!("Finished decompressing images");
    Ok(())

}

fn decompress_one_image(image: ResourceImage) -> JoinHandle<Result<()>> {
    thread::spawn(move || {
        let start = Instant::now();
        info!("Decompressing {}", image.path().display());
        image.decompress()?;
        cmd!("/usr/bin/du", "-h {}", image.path().display())?;
        info!("Decompress {:?} finished in {} seconds",
              image.path().file_name().unwrap(),
              start.elapsed().as_secs());
        Ok(())
    })
}
