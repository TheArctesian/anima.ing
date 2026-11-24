//! The K8 bodies between `anima-daemon` and the hub (MET-012, MET-013, MET-014, MET-017); unknown members are ignored (K8 Section 1).

mod agent_usage;
mod batch;
mod batteries;
mod bodies;
mod check;
mod inventory;
mod keys;
mod refusal;
mod secrets;
#[cfg(test)]
mod tests;
mod thread_summaries;
mod times;
mod update;
mod work;

pub use agent_usage::{BatchAccount, BatchAgentUsage, BatchLimit, BatchModel, BatchSessionDay};
pub use batch::{
    Announcements, Batch, BatchDrive, BatchGpu, BatchPoint, BatchSeries, DriveKind, GpuVendor,
};
pub use batteries::{BatchBattery, BatteryState};
pub use bodies::{
    DEVICE_NAME_CHARS, EnrolRequest, Enrolled, RenewRequest, Renewed, is_device_name,
};
pub use check::Accepted;
pub use inventory::{BatchInventory, BatchTool};
pub use keys::{
    BatchKeys, FetchedKey, KeyFetch, KeyValue, KeyVersion, KeysCurrent, MAX_KEYS, MAX_VALUE_BYTES,
    PushAccepted, is_key_name, versions_fit,
};
pub use refusal::{Refusal, RefusalCode};
pub use secrets::{CODE_BYTES, CodeHash, DaemonToken, EnrolmentCode, TOKEN_BYTES, TokenHash};
pub use thread_summaries::entries::{
    MAX_THREAD_ENTRIES, MAX_THREAD_ENTRY, MAX_THREAD_UPLOAD, ThreadEntry, ThreadUpload,
    ThreadUploadReceipt,
};
pub use thread_summaries::{
    ThreadSummaries, ThreadSummariesReceipt, ThreadSummary, ThreadSummaryReceipt,
};
pub use update::{
    BatchUpdate, MAX_RECORD_BYTES, MAX_SIGNATURE_BYTES, MAX_TRIES, ReleaseRecord, TryCause,
    TryResult, UpdateInstruction, UpdateRequest, UpdateTry,
};
pub use work::{
    Claim, Cleanup, CopyOutcome, FileHash, FoundEntry, Free, Held, Lease, Limits, LoadEntry,
    LogPart, MAX_LOG_PART_BYTES, MODEL_TYPES, ModelOutcome, ModelState, Progress, Reported,
    SecretRequest, ServerLoads, StopOrder, StopReason, Switch, WorkArchitecture, WorkJob, WorkKind,
    WorkOs, WorkReport, WorkRepository, WorkResult, WorkState, WorkType,
};
