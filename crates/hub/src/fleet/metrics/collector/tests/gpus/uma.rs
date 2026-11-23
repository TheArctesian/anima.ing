use super::{amd_card, answer, found, gpus, output};
use crate::fleet::metrics::GpuVendor;

#[test]
fn met_007_ssh_uma_setting_belongs_only_to_its_card_even_when_optional_files_are_missing() {
    let mut printed = amd_card(1);
    printed.extend(amd_card(2));
    printed.extend(b"/sys/class/drm/card1/device/uma/carveout:0\n");
    let read = answer(GpuVendor::Amd, &output(Some(2), &printed));
    let cards = gpus(&read);
    assert_eq!(cards[0].busy_percent, Some(37.0));
    assert_eq!(cards[0].vram, None);
    assert!(cards[1].vram.is_some());
    let discovery = found(&[(GpuVendor::Amd, read)]);
    assert!(!discovery.gpus[0].gpu.has_vram);
    assert!(discovery.gpus[1].gpu.has_vram);
}
