use proptest::prelude::*;

/// Unicode text without NUL, which guacd reads as the end of an element.
pub fn text(max_chars: usize) -> impl Strategy<Value = String> {
    let non_nul = any::<char>().prop_filter("no NUL", |c| *c != '\0');
    prop::collection::vec(non_nul, 0..=max_chars).prop_map(String::from_iter)
}
