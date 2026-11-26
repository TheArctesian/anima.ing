use daemon_core::artifact::Offer;
use uuid::Uuid;

pub(super) fn directory(offer: &Offer) -> String {
    attempt_directory(offer.job, offer.attempt)
}

pub(super) fn attempt_directory(job: Uuid, attempt: u32) -> String {
    format!("{job}-{attempt}")
}

pub(super) fn encoded_digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut digest = String::from("sha256:");
    for byte in bytes {
        digest.push(char::from(HEX[usize::from(byte >> 4)]));
        digest.push(char::from(HEX[usize::from(byte & 15)]));
    }
    digest
}
