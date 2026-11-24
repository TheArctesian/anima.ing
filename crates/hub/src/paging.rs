//! Cursor pagination for every API list (API-006, K1, R-014).
//! async-graphql's connection type adds backward paging that K1 lacks; domains use this type.

use async_graphql::SimpleObject;

/// Where a page ends and whether another page follows. Exposed as K1's `PageInfo`; the Rust
/// name avoids the banned word "info" (CQ-030).
#[derive(Clone, Debug, Default, PartialEq, Eq, SimpleObject)]
#[graphql(name = "PageInfo")]
pub struct PageEnd {
    pub has_next_page: bool,
    /// The cursor of the last edge on this page; `None` when the page is empty.
    pub end_cursor: Option<String>,
}
