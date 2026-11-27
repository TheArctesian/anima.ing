use super::{Cpu, batteries};
use std::{fs, path::Path};

#[test]
fn cpu_usage_needs_two_consistent_aggregate_samples() {
    let previous = Cpu::from_stat("cpu 10 0 10 80 0 0 0 0\ncpu0 1").unwrap();
    let current = Cpu::from_stat("cpu 30 0 10 160 0 0 0 0").unwrap();
    assert_eq!(current.usage(previous), Some(20.0));
    assert_eq!(previous.usage(current), None);
    assert_eq!(current.usage(current), None);
    assert!(Cpu::from_stat("cpu 30 0 10 unknown 0 0 0 0").is_none());
}

#[test]
fn system_batteries_distinguish_missing_charge_and_absent_hardware() {
    let directory = tempfile::tempdir().unwrap();
    let supply = directory.path();
    let laptop = supply.join("BAT0");
    let peripheral = supply.join("mouse");
    write_supply(
        &laptop,
        &[
            ("type", "Battery"),
            ("capacity", "73"),
            ("status", "Charging"),
        ],
    );
    write_supply(&peripheral, &[("type", "Battery"), ("scope", "Device")]);
    let first = batteries(supply).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].percent, Some(73));
    fs::remove_file(laptop.join("capacity")).unwrap();
    assert_eq!(batteries(supply).unwrap()[0].percent, None);
    fs::write(laptop.join("present"), "0").unwrap();
    assert!(batteries(supply).unwrap().is_empty());
}

fn write_supply(path: &Path, fields: &[(&str, &str)]) {
    fs::create_dir(path).unwrap();
    for (name, value) in fields {
        fs::write(path.join(name), value).unwrap();
    }
}
