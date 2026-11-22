//! The files of a prompt in the line of Claude Code (TH111).

use std::path::PathBuf;

use serde_json::json;

use crate::control::attach::ImageKind;

use super::*;

#[test]
fn th111_claude_code_gets_inline_images_and_pdfs_as_blocks_and_the_rest_by_path() {
    let file = |name: &str, kind| Attachment {
        name: name.to_owned(),
        path: PathBuf::from(format!("/a/{name}")),
        kind,
    };
    let prompt = Prompt {
        text: "look".to_owned(),
        attachments: vec![
            file(
                "a.png",
                Attached::Image {
                    kind: ImageKind::Png,
                    inline: Some("AAAA".to_owned()),
                },
            ),
            file(
                "b.pdf",
                Attached::Pdf {
                    inline: Some("BBBB".to_owned()),
                },
            ),
            file(
                "big.jpg",
                Attached::Image {
                    kind: ImageKind::Jpeg,
                    inline: None,
                },
            ),
            file("c.txt", Attached::File),
        ],
    };
    let sent = line(&ClaudeInput::prompt_with(&prompt));
    let content = sent["message"]["content"].as_array().expect("blocks");
    let words = content[0]["text"].as_str().expect("words");
    assert!(words.contains("\n- /a/big.jpg") && words.contains("\n- /a/c.txt"));
    assert!(!words.contains("a.png") && !words.contains("b.pdf"));
    assert_eq!(
        content[1],
        json!({"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "AAAA"}})
    );
    assert_eq!(content[2]["type"], "document");
    assert_eq!(content[2]["source"]["media_type"], "application/pdf");
    assert_eq!(content.len(), 3);
    assert_eq!(
        line(&ClaudeInput::prompt_with(&Prompt::plain("hi"))),
        line(&ClaudeInput::prompt("hi")),
        "a prompt with no file is the line of before"
    );
}
