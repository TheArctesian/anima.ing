mod account_keys;
mod addresses;
mod health_table;
mod keys;
mod registration_paths;
mod schedule_times;
mod tokens;

use std::sync::LazyLock;

use rsa::{
    RsaPrivateKey,
    pkcs1::{EncodeRsaPrivateKey as _, LineEnding},
    rand_core::OsRng,
};

/// Made in this test process, never a key file.
pub static TEST_KEY: LazyLock<String> = LazyLock::new(|| pem_of_bits(2048));

pub fn pem_of_bits(bits: usize) -> String {
    let key = RsaPrivateKey::new(&mut OsRng, bits).unwrap();
    key.to_pkcs1_pem(LineEnding::LF).unwrap().to_string()
}
