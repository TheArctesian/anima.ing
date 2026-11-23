use std::path::PathBuf;

use crate::model_work::cause::Cause;
use crate::model_work::command::vllm_arguments;
use crate::model_work::files::code::{
    CopyFiles, MAX_CONFIG_BYTES, MAX_TEMPLATE_BYTES, check_config, check_copy,
    check_template_length, check_tokenizer_config,
};
use crate::model_work::names::Runtime;

const REFUSED: Result<(), Cause> = Err(Cause::RemoteCode);

#[test]
fn a_plain_config_is_accepted() {
    let config =
        br#"{"architectures":["LlamaForCausalLM"],"model_type":"llama","rope":{"auto":1}}"#;
    assert_eq!(check_config(config), Ok(()));
    assert_eq!(check_copy(Runtime::MlxLm, &CopyFiles::default()), Ok(()));
}

#[test]
fn a_member_of_remote_code_is_refused_at_any_depth() {
    for text in [
        r#"{"auto_map":{"AutoModel":"modeling.Model"}}"#,
        r#"{"custom_pipelines":{}}"#,
        r#"{"auto_map":null}"#,
        r#"{"text_config":{"auto_map":{}}}"#,
        r#"{"a":[1,{"b":[[{"custom_pipelines":1}]]}]}"#,
        r#"{"x":1,"auto_map":1,"x":2}"#,
        r#"{"auto_map":null,"auto_map":{"AutoModel":"m.M"}}"#,
    ] {
        assert_eq!(check_config(text.as_bytes()), REFUSED, "{text}");
        assert_eq!(check_tokenizer_config(text.as_bytes()), REFUSED, "{text}");
    }
    // A value that only names the member is not the member.
    assert_eq!(check_config(br#"{"note":"auto_map"}"#), Ok(()));
}

#[test]
fn a_file_that_does_not_read_as_an_object_is_refused() {
    for text in ["", "[]", "\"auto\"", "{", "{\"a\":1} x", "\u{feff}{}"] {
        assert_eq!(check_config(text.as_bytes()), REFUSED, "{text:?}");
    }
    let deep = format!("{}{}", "[".repeat(200), "]".repeat(200));
    assert_eq!(
        check_config(format!("{{\"a\":{deep}}}").as_bytes()),
        REFUSED
    );
}

#[test]
fn an_oversized_file_is_refused_before_it_is_parsed() {
    let mut big = b"{\"a\":\"".to_vec();
    big.resize(MAX_CONFIG_BYTES, b'x');
    big.extend_from_slice(b"\"}");
    assert_eq!(check_config(&big), REFUSED);
    assert_eq!(check_tokenizer_config(&big), REFUSED);
}

#[test]
fn a_chat_template_is_at_most_64_kib() {
    let limit = usize::try_from(MAX_TEMPLATE_BYTES).expect("fits");
    let template = |length: usize| format!("{{\"chat_template\":\"{}\"}}", "a".repeat(length));
    assert_eq!(check_tokenizer_config(template(limit).as_bytes()), Ok(()));
    assert_eq!(
        check_tokenizer_config(template(limit + 1).as_bytes()),
        REFUSED
    );
    let list = |length: usize| {
        let half = "a".repeat(length / 2);
        format!(
            "{{\"chat_template\":[{{\"name\":\"default\",\"template\":\"{half}\"}},\
             {{\"name\":\"tool_use\",\"template\":\"{half}a\"}}]}}"
        )
    };
    assert_eq!(check_tokenizer_config(list(limit - 2).as_bytes()), Ok(()));
    assert_eq!(check_tokenizer_config(list(limit).as_bytes()), REFUSED);
    for odd in [
        r#"{"chat_template":5}"#,
        r#"{"chat_template":[{"name":"x"}]}"#,
        r#"{"chat_template":{"template":"x"}}"#,
    ] {
        assert_eq!(check_tokenizer_config(odd.as_bytes()), REFUSED, "{odd}");
    }
    assert_eq!(check_tokenizer_config(br#"{"chat_template":null}"#), Ok(()));
    assert_eq!(check_template_length(MAX_TEMPLATE_BYTES), Ok(()));
    assert_eq!(check_template_length(MAX_TEMPLATE_BYTES + 1), REFUSED);
    let over = CopyFiles {
        json: &[],
        template_file: Some(MAX_TEMPLATE_BYTES + 1),
    };
    assert_eq!(check_copy(Runtime::MlxLm, &over), REFUSED);
}

#[test]
fn vllm_loads_only_safetensors_and_never_remote_code() {
    let copy = std::env::temp_dir().join("anima-copy");
    let words = vllm_arguments(&copy, 41000);
    let copy = PathBuf::from(&copy).display().to_string();
    assert_eq!(words[..2], ["serve".to_owned(), copy.clone()]);
    let after = |flag: &str| {
        let at = words.iter().position(|word| word == flag).expect(flag);
        words[at + 1].clone()
    };
    assert_eq!(after("--load-format"), "safetensors");
    assert_eq!(after("--tokenizer"), copy);
    assert_eq!(after("--host"), "127.0.0.1");
    assert_eq!(words.iter().filter(|w| *w == "--load-format").count(), 1);
    assert!(words.iter().all(|word| !word.contains("trust-remote-code")));
    assert!(words.iter().all(|word| !word.contains("key")));
    assert!(words.contains(&"--disable-log-requests".to_owned()));
}
