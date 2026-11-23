//! The K1 custom scalars, kept as the hub's text so `--json` prints its exact value.

/// RFC 3339 in UTC (K7).
pub type DateTime = String;
pub type IpAddr = String;
pub type Cidr = String;
/// `graphql_client` looks the alias up by the scalar's own name.
pub type JSON = serde_json::Value;
