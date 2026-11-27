use super::{Error, Plan, Result, command, disks, persist, progress, volumes};
use std::{fs, path::Path};

const ROOT: &str = "/mnt/anima-install";

struct Mounts {
    mounted: bool,
}
impl Mounts {
    fn clean(&mut self) -> Result<()> {
        if self.mounted {
            command::run("umount", &["--recursive", ROOT])?;
            self.mounted = false;
        }
        fs::remove_dir(ROOT)?;
        Ok(())
    }
}
impl Drop for Mounts {
    fn drop(&mut self) {
        if self.mounted && command::run("umount", &["--recursive", ROOT]).is_err() {
            eprintln!(
                "Installation mounts remain busy. Reboot the live environment before another installation."
            );
        }
        let _ = fs::remove_dir(ROOT);
    }
}

pub fn run(plan: &Plan) -> Result<()> {
    if !rustix::process::geteuid().is_root() || !Path::new("/sys/firmware/efi").is_dir() {
        return Err(Error::Refused(
            "Installation requires root in the live UEFI environment.",
        ));
    }
    let _writer = persist::writer_lock()?;
    if Path::new(ROOT).exists()
        || !plan.system.starts_with("/nix/store")
        || plan.system.canonicalize()? != plan.system
        || !plan.system.join("bin/switch-to-configuration").is_file()
        || !plan.system.join("etc/anima-preview").is_file()
    {
        return Err(Error::Refused(
            "The installation mount is busy or the trusted offline system closure is missing.",
        ));
    }
    disks::validate_all(&plan.selected, &plan.data)?;
    progress("Validated selected disks", 0.05);
    let previous = rustix::process::umask(rustix::fs::Mode::from_raw_mode(0o022));
    let result = execute(plan);
    rustix::process::umask(previous);
    result
}

fn execute(plan: &Plan) -> Result<()> {
    let root = Path::new(ROOT);
    if !Path::new("/mnt").exists() {
        persist::directory(Path::new("/mnt"), 0o755)?;
    }
    persist::directory(root, 0o755)?;
    let mut mounts = Mounts { mounted: false };
    disks::validate(&plan.selected, &disks::inventory()?, true)?;
    progress("Partitioning selected system disk", 0.15);
    let parts = partition(&plan.selected)?;
    mount_system(&parts, &mut mounts)?;
    progress("Writing private owner settings", 0.4);
    owner(plan)?;
    volumes::prepare(&plan.data, &root.join("persist"))?;
    progress("Installing verified local system closure", 0.7);
    let system = plan
        .system
        .to_str()
        .ok_or(Error::Refused("Invalid system path."))?;
    command::run(
        "nixos-install",
        &[
            "--root",
            ROOT,
            "--system",
            system,
            "--no-root-passwd",
            "--no-channel-copy",
            "--option",
            "substituters",
            "",
        ],
    )?;
    progress("Retaining local connection settings", 0.9);
    persist::network(&root.join("persist/network-connections"))?;
    persist::display(&root.join("persist/display.json"))?;
    persist::reduced_motion(&root.join("persist/anima-reduced-motion"))?;
    command::run("sync", &[])?;
    mounts.clean()?;
    println!(
        "{}",
        serde_json::json!({"state":"succeeded", "progress":1.0, "event":"ANIMA_INSTALL_COMPLETE"})
    );
    Ok(())
}

fn mount_system(parts: &[String], mounts: &mut Mounts) -> Result<()> {
    let root = Path::new(ROOT);
    command::run("mount", &["-t", "btrfs", &parts[1], ROOT])?;
    mounts.mounted = true;
    for name in ["root", "nix", "var", "persist"] {
        command::run(
            "btrfs",
            &["subvolume", "create", &format!("{ROOT}/@{name}")],
        )?;
    }
    command::run("umount", &[ROOT])?;
    mounts.mounted = false;
    command::run(
        "mount",
        &[
            "-t",
            "btrfs",
            "-o",
            "subvol=@root,compress=zstd",
            &parts[1],
            ROOT,
        ],
    )?;
    mounts.mounted = true;
    for name in ["nix", "var", "persist", "boot"] {
        persist::directory(&root.join(name), 0o755)?;
    }
    for name in ["nix", "var", "persist"] {
        command::run(
            "mount",
            &[
                "-t",
                "btrfs",
                "-o",
                &format!("subvol=@{name},compress=zstd"),
                &parts[1],
                &format!("{ROOT}/{name}"),
            ],
        )?;
    }
    command::run(
        "mount",
        &[
            "-t",
            "vfat",
            "-o",
            "umask=0077",
            &parts[0],
            &format!("{ROOT}/boot"),
        ],
    )
}

fn owner(plan: &Plan) -> Result<()> {
    let root = Path::new(ROOT);
    persist::directory(&root.join("persist/network-connections"), 0o700)?;
    persist::directory(&root.join("persist/home"), 0o755)?;
    let password = zeroize::Zeroizing::new(format!("{}\n", plan.hashed_password));
    persist::private_write(&root.join("persist/owner-password"), password.as_bytes())?;
    persist::private_write(&root.join("persist/console.map"), plan.keymap.as_bytes())
}

fn partition(selected: &disks::Disk) -> Result<Vec<String>> {
    let disk = selected.path.as_str();
    command::run("wipefs", &["--all", disk])?;
    command::run(
        "parted",
        &[
            "--script", disk, "mklabel", "gpt", "mkpart", "ESP", "fat32", "1MiB", "1025MiB", "set",
            "1", "esp", "on", "mkpart", "Anima", "btrfs", "1025MiB", "100%",
        ],
    )?;
    command::run("udevadm", &["settle"])?;
    let parts = disks::partitions(selected, 2)?;
    progress("Creating selected filesystems", 0.25);
    command::run("mkfs.vfat", &["-n", "ANIMA_EFI", &parts[0]])?;
    command::run("mkfs.btrfs", &["-f", "-L", "ANIMA_OS", &parts[1]])?;
    Ok(parts)
}
