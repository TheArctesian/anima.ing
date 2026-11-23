/// Why a key file could not be opened or a file signed or checked; it carries no secret (LOG-007).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SignatureError {
    #[error("the file is not a minisign key")]
    Format,
    #[error("the key file is encrypted and needs its password")]
    PasswordNeeded,
    #[error("the password does not open the key file")]
    WrongPassword,
    #[error("the file could not be signed or the signature does not verify")]
    Signature,
    #[error("the signature is not for this version")]
    SignedVersion,
}
