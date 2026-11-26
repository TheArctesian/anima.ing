//! Separate short Compose fields without treating a variable's default as a separator.

pub(super) fn first(value: &str, delimiter: char) -> Option<(&str, &str)> {
    let offset = offsets(value, delimiter).next()?;
    Some((&value[..offset], &value[offset + delimiter.len_utf8()..]))
}

pub(super) fn last(value: &str, delimiter: char) -> Option<(&str, &str)> {
    let offset = offsets(value, delimiter).last()?;
    Some((&value[..offset], &value[offset + delimiter.len_utf8()..]))
}

fn offsets(value: &str, delimiter: char) -> impl Iterator<Item = usize> {
    let mut cursor = 0;
    std::iter::from_fn(move || {
        while let Some(next) = value[cursor..].find([delimiter, '$']) {
            let offset = cursor + next;
            if value.as_bytes()[offset] != b'$' {
                cursor = offset + delimiter.len_utf8();
                return Some(offset);
            }
            cursor = offset + 1;
            let rest = &value[cursor..];
            if rest.starts_with('$') {
                cursor += 1;
            } else if rest.starts_with('{') {
                cursor += rest.find('}').map_or(rest.len(), |end| end + 1);
            }
        }
        None
    })
}
