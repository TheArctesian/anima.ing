pub(super) fn identifier(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 192
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

pub(super) fn package(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 192
        && text.split(';').all(identifier)
        && (matches!(text, "platform-tools" | "emulator")
            || [
                "platforms;",
                "system-images;",
                "build-tools;",
                "cmdline-tools;",
            ]
            .iter()
            .any(|prefix| text.starts_with(prefix)))
}
