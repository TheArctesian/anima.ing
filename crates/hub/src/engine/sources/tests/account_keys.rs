//! K-E1 Section 7b, R-261.

use crate::engine::sources::acquisition::account_key::{
    Refusal, base, from_secret, instance_key, key, to_secret,
};

#[test]
fn dawarich_key_a_base_is_an_https_origin_and_nothing_more() {
    let origin = |text: &str| base(text).map(|url| instance_key(&url));
    assert_eq!(
        origin(" https://Dawarich.Example "),
        Ok("https://dawarich.example".to_owned())
    );
    assert_eq!(
        origin("https://10.0.0.5:3000/"),
        Ok("https://10.0.0.5:3000".to_owned())
    );
    assert_eq!(
        origin("https://dawarich.example:443"),
        Ok("https://dawarich.example".to_owned())
    );
    for refused in [
        "https://user:pass@dawarich.example",
        "https://user@dawarich.example",
        "https://dawarich.example/api",
        "https://dawarich.example?x=1",
        "https://dawarich.example#top",
        "dawarich.example",
        "",
    ] {
        assert_eq!(origin(refused), Err(Refusal::Base), "{refused}");
    }
    assert_eq!(
        origin("http://192.168.1.20:3000"),
        Ok("http://192.168.1.20:3000".to_owned()),
        "the transport decides where plain http may go (OD-E26)"
    );
}

#[test]
fn dawarich_key_a_key_is_one_word_of_at_most_512_characters() {
    assert_eq!(key(" abc123 "), Ok("abc123"));
    for refused in ["", "   ", "two words", "line\nbreak", &"k".repeat(513)] {
        assert_eq!(key(refused), Err(Refusal::Key));
    }
}

#[test]
fn dawarich_key_a_stored_key_opens_only_for_the_base_of_its_account() {
    let url = base("https://dawarich.example").unwrap();
    let secret = to_secret(&url, "the-key");
    let (opened, api_key) = from_secret(&secret, "https://dawarich.example").unwrap();
    assert_eq!(instance_key(&opened), "https://dawarich.example");
    assert_eq!(api_key.expose(), b"the-key");
    // A database writer points the account at a different server: the key does not open.
    assert!(from_secret(&secret, "https://other.example").is_none());
    let unknown = crate::fleet::credentials::Secret::new(b"{\"v\":2}".to_vec());
    assert!(from_secret(&unknown, "https://dawarich.example").is_none());
}
