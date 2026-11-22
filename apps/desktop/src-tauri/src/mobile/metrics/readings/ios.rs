#[expect(
    unsafe_code,
    reason = "the iOS host memory reader is compiled by the Go C bridge"
)]
unsafe extern "C" {
    fn AnimaLinkMemory(total: *mut u64, used: *mut u64) -> std::ffi::c_int;
}

#[expect(
    unsafe_code,
    reason = "AnimaLinkMemory writes the sample to its two caller-owned outputs"
)]
pub(super) fn memory() -> Option<(u64, u64)> {
    let (mut total, mut used) = (0, 0);
    // SAFETY: both pointers are distinct, writable u64s and the call does not retain them.
    let success = unsafe { AnimaLinkMemory(&raw mut total, &raw mut used) };
    (success != 0 && total > 0 && used <= total).then_some((total, used))
}
