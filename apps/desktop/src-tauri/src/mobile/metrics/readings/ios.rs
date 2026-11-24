use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

#[expect(
    unsafe_code,
    reason = "the iOS host readers are compiled by the Go C bridge"
)]
unsafe extern "C" {
    fn AnimaLinkCpuTicks(ticks: *mut u32) -> std::ffi::c_int;
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

const MAX_SAMPLE_GAP: Duration = Duration::from_secs(60);
static LAST_CPU: Mutex<Option<(Instant, [u32; 4])>> = Mutex::new(None);

#[expect(
    unsafe_code,
    reason = "AnimaLinkCpuTicks fills four caller-owned counters"
)]
pub(super) fn cpu_percent() -> Option<f64> {
    let mut previous = LAST_CPU.lock().ok()?;
    let mut ticks = [0; 4];
    // SAFETY: the array contains four writable u32s and the call does not retain the pointer.
    if unsafe { AnimaLinkCpuTicks(ticks.as_mut_ptr()) } == 0 {
        return None;
    }
    let now = Instant::now();
    let (before, old) = previous.replace((now, ticks))?;
    if now.duration_since(before) > MAX_SAMPLE_GAP {
        return None;
    }
    let [user, system, idle, nice] =
        std::array::from_fn(|index| f64::from(ticks[index].wrapping_sub(old[index])));
    let busy = user + system + nice;
    let total = busy + idle;
    (total > 0.0).then(|| 100.0 * busy / total)
}
