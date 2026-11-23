//! Whether a JSON key (or label value) names a secret (LOG-007, D-3).

use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

/// A normalised name that contains one of these names a secret.
const SECRET_PARTS: [&str; 21] = [
    "password",
    "passwd",
    "passphrase",
    "passcode",
    "secret",
    "token",
    "jwt",
    "authorization",
    "cookie",
    "bearer",
    "privatekey",
    "privkey",
    "authkey",
    "basicauth",
    "proxyauth",
    "keymaterial",
    "totp",
    "otpauth",
    "recoverycode",
    "backupcode",
    "mfacode",
];

/// A name with one of these as a whole word names a secret (`otpCode`), while `notPaused` does not.
/// `auth` is not one: `authType` names a method, and a key named `auth` is decided by its value.
const SECRET_WORDS: [&str; 7] = ["otp", "pem", "pin", "pass", "pwd", "pw", "psk"];

/// With one of these in the name, `pin` means certificate pinning, not a PIN (`certPin`).
const PINNING: [&str; 3] = ["cert", "tls", "spki"];

/// One-word names that end in "key"; a name whose last word is `key` (`apiKey`) is a secret too.
const KEY_COMPOUNDS: [&str; 5] = [
    "apikey",
    "sshkey",
    "accesskey",
    "signingkey",
    "encryptionkey",
];

/// With one of these and `key` in the name, it names a private key, even one that ends like a public key.
const PRIVATE: [&str; 2] = ["private", "priv"];

/// Names ending in these are public keys, kept as audit evidence (SEC-006, WEB-009), unless a secret part is there.
const PUBLIC_KEYS: [&str; 4] = ["hostkey", "publickey", "nodekey", "authorizedkey"];

/// What a name says about its value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Secrecy {
    Plain,
    /// Holds credentials: each scalar under it is a secret, but credential metadata (`kind`, `username`, …).
    Holder,
    Secret,
}

#[must_use]
pub fn secrecy(name: &str) -> Secrecy {
    let words = words(name);
    let joined = words.concat();
    let has_word = |wanted: &str| words.iter().any(|word| word == wanted);
    let pinning = PINNING.iter().any(|part| joined.contains(part));
    let last_is = |wanted: &str| words.last().is_some_and(|word| word == wanted);
    let has_secret_part = (joined.contains("key")
        && PRIVATE.iter().any(|part| joined.contains(part)))
        || SECRET_PARTS.iter().any(|part| joined.contains(part))
        || SECRET_WORDS
            .iter()
            .any(|word| has_word(word) && !(*word == "pin" && pinning));
    if has_secret_part {
        Secrecy::Secret
    } else if PUBLIC_KEYS.iter().any(|public| joined.ends_with(public)) {
        Secrecy::Plain
    } else if last_is("key")
        || KEY_COMPOUNDS
            .iter()
            .any(|compound| joined.ends_with(compound))
    {
        Secrecy::Secret
    } else if (joined.contains("credential") || has_word("cred")) && !last_is("id") {
        Secrecy::Holder
    } else {
        Secrecy::Plain
    }
}

/// The name in lower-case ASCII letters and digits only.
#[must_use]
pub fn normalise(name: &str) -> String {
    words(name).concat()
}

/// Lower-case ASCII words: NFKD drops marks (`pässword`), and a separator or a case change ends a word.
/// A plural `s` on the last word is dropped, but not a double `ss` (`pass`).
fn words(name: &str) -> Vec<String> {
    let chars: Vec<char> = name.nfkd().filter(|c| !is_combining_mark(*c)).collect();
    let mut words = Vec::new();
    let mut word = String::new();
    let mut after_lower = false;
    for (index, &c) in chars.iter().enumerate() {
        let before_lower = chars.get(index + 1).is_some_and(|next| next.is_lowercase());
        if c.is_uppercase() && (after_lower || before_lower) && !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
        let folded: String = c
            .to_lowercase()
            .filter(char::is_ascii_alphanumeric)
            .collect();
        if folded.is_empty() && !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
        word.push_str(&folded);
        after_lower = !folded.is_empty() && !c.is_uppercase();
    }
    if !word.is_empty() {
        words.push(word);
    }
    if let Some(last) = words
        .last_mut()
        .filter(|last| last.ends_with('s') && !last.ends_with("ss"))
    {
        last.pop();
    }
    words
}
