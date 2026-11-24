mod disclosure;
mod host;
mod key;
mod layout;
mod password;
mod sealing;
mod source;
mod ssh;

use zeroize::Zeroizing;

use super::{Secret, Vault};

const CONTEXT: &[u8] = b"test/context";

fn test_key() -> [u8; 32] {
    std::array::from_fn(|index| u8::try_from(index).unwrap())
}

fn vault_from(key: [u8; 32]) -> Vault {
    Vault::from_key(Zeroizing::new(key))
}

fn other_vault() -> Vault {
    vault_from([0xa5; 32])
}

fn secret(text: &str) -> Secret {
    Secret::from(text.to_owned())
}
