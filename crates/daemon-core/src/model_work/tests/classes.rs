use std::fmt::Write as _;

use crate::model_work::cause::Cause;
use crate::model_work::files::classes::{VLLM_ARCHITECTURES, architectures};
use crate::model_work::files::code::{CopyFiles, check_copy};
use crate::model_work::names::Runtime;

const REFUSED: Result<(), Cause> = Err(Cause::RemoteCode);

fn vllm(json: &[(&str, &[u8])]) -> Result<(), Cause> {
    check_copy(
        Runtime::Vllm,
        &CopyFiles {
            json,
            template_file: None,
        },
    )
}

#[test]
fn a_known_class_passes_and_any_other_is_refused() {
    assert_eq!(VLLM_ARCHITECTURES.len(), 131);
    let ok = br#"{"architectures":["LlamaForCausalLM"]}"#;
    assert_eq!(vllm(&[("config.json", ok)]), Ok(()));
    let tokenizer = br#"{"tokenizer_class":"LlamaTokenizerFast"}"#;
    assert_eq!(vllm(&[("tokenizer_config.json", tokenizer)]), Ok(()));
    for config in [
        r#"{"architectures":["EvilForCausalLM"]}"#,
        r#"{"architectures":["LlamaForCausalLM","EvilForCausalLM"]}"#,
        r#"{"architectures":["modeling_evil.LlamaForCausalLM"]}"#,
        r#"{"architectures":["org/repo--LlamaForCausalLM"]}"#,
        r#"{"architectures":["a:LlamaForCausalLM"]}"#,
        r#"{"architectures":"LlamaForCausalLM"}"#,
        r#"{"architectures":[1]}"#,
        r#"{"architectures":[]}"#,
        r#"{"architectures":null}"#,
        r#"{"model_type":"llama"}"#,
    ] {
        assert_eq!(
            vllm(&[("config.json", config.as_bytes())]),
            REFUSED,
            "{config}"
        );
    }
    for tokenizer in [
        r#"{"tokenizer_class":"EvilTokenizer"}"#,
        r#"{"tokenizer_class":"tokenization_evil.EvilTokenizer"}"#,
        r#"{"tokenizer_class":["LlamaTokenizerFast"]}"#,
    ] {
        let bytes = tokenizer.as_bytes();
        assert_eq!(
            vllm(&[("tokenizer_config.json", bytes)]),
            REFUSED,
            "{tokenizer}"
        );
    }
}

#[test]
fn llama_server_reads_no_class_list() {
    assert_eq!(architectures(Runtime::LlamaServer), None);
    let files = CopyFiles {
        json: &[("config.json", br#"{"architectures":["EvilForCausalLM"]}"#)],
        template_file: None,
    };
    assert_eq!(check_copy(Runtime::LlamaServer, &files), Ok(()));
}

#[test]
fn each_checked_json_file_is_walked_and_no_other_name_is_taken() {
    let bad: &[u8] = br#"{"x":{"auto_map":{}}}"#;
    for name in [
        "generation_config.json",
        "special_tokens_map.json",
        "preprocessor_config.json",
        "model.safetensors.index.json",
        "sub/preprocessor_config.json",
    ] {
        assert_eq!(vllm(&[(name, bad)]), REFUSED, "{name}");
        assert_eq!(vllm(&[(name, b"{}")]), Ok(()), "{name}");
    }
    assert_eq!(vllm(&[("tokenizer.json", b"{}")]), REFUSED);
    assert_eq!(vllm(&[(".safetensors.index.json", b"{}")]), REFUSED);
}

#[test]
fn a_tokenizer_config_of_gemma_3_size_passes() {
    let mut text = String::from(r#"{"tokenizer_class":"GemmaTokenizer","added_tokens_decoder":{"#);
    for id in 0..12_000 {
        if id > 0 {
            text.push(',');
        }
        write!(
            text,
            r#""{id}":{{"content":"<unused{id}>","lstrip":false,"normalized":false,"rstrip":false,"single_word":false,"special":true}}"#
        )
        .expect("writes");
    }
    text.push_str(r#"},"chat_template":"{{ bos_token }}"}"#);
    assert!(
        (1_100_000..2_000_000).contains(&text.len()),
        "{}",
        text.len()
    );
    let config: &[u8] = br#"{"architectures":["Gemma3ForCausalLM"]}"#;
    let json = [
        ("config.json", config),
        ("tokenizer_config.json", text.as_bytes()),
    ];
    assert_eq!(vllm(&json), Ok(()));
}
