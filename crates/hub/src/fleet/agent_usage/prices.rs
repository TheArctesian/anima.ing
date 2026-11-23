//! Token cost estimates from a fixed table of list prices (AIS-021); the hub never looks a price up on the network.
//! The estimate is not the bill: each cache write is priced as a five-minute write, and long-context surcharges are not counted.

use super::model::TokenSums;

/// The day on which the prices of [`PRICES`] were the public list prices, `YYYY-MM-DD`.
pub const PRICES_AS_OF: &str = "2026-11-17";

/// US dollars per million tokens.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Price {
    /// The model name with no date suffix.
    model: &'static str,
    input: f64,
    output: f64,
    cache_read: f64,
    cache_write: f64,
}

/// Anthropic: a cache read costs a tenth of the input, a five-minute cache write a quarter more.
const fn claude(model: &'static str, input: f64, output: f64) -> Price {
    Price {
        model,
        input,
        output,
        cache_read: input / 10.0,
        cache_write: input * 1.25,
    }
}

const fn claude_read(model: &'static str, input: f64, output: f64, cache_read: f64) -> Price {
    Price {
        model,
        input,
        output,
        cache_read,
        cache_write: input * 1.25,
    }
}

const fn openai(model: &'static str, input: f64, output: f64, cache_read: f64) -> Price {
    Price {
        model,
        input,
        output,
        cache_read,
        cache_write: input,
    }
}

/// Only prices known for sure, as of [`PRICES_AS_OF`]. Claude Haiku 5.5 has no entry: its price
/// depends on prompt length, which the counts do not hold. `OpenAI` models take short-context prices.
const PRICES: [Price; 43] = [
    claude_read("claude-fable-5-2", 8.0, 40.0, 0.2),
    claude_read("claude-fable-5-1", 10.0, 50.0, 0.25),
    claude("claude-fable-5", 10.0, 50.0),
    claude_read("claude-opus-5-5", 4.0, 20.0, 0.2),
    claude("claude-opus-5", 5.0, 25.0),
    claude("claude-opus-4-8", 5.0, 25.0),
    claude("claude-opus-4-7", 5.0, 25.0),
    claude("claude-opus-4-6", 5.0, 25.0),
    claude("claude-opus-4-5", 5.0, 25.0),
    claude("claude-opus-4-1", 15.0, 75.0),
    claude("claude-opus-4", 15.0, 75.0),
    claude_read("claude-sonnet-5-5", 2.0, 10.0, 0.1),
    claude("claude-sonnet-5", 2.0, 10.0),
    claude("claude-sonnet-4-6", 3.0, 15.0),
    claude("claude-sonnet-4-5", 3.0, 15.0),
    claude("claude-sonnet-4", 3.0, 15.0),
    claude("claude-3-7-sonnet", 3.0, 15.0),
    claude("claude-haiku-4-5", 1.0, 5.0),
    claude("claude-3-5-haiku", 0.8, 4.0),
    openai("gpt-6.2-astra", 8.0, 40.0, 0.4),
    openai("gpt-6.2-sol", 2.0, 10.0, 0.1),
    openai("gpt-6.1-sol", 2.0, 10.0, 0.1),
    openai("gpt-6.1-terra", 1.0, 5.0, 0.05),
    openai("gpt-6-astra", 10.0, 50.0, 1.0),
    openai("gpt-6-sol", 2.0, 10.0, 0.2),
    openai("gpt-6-luna", 0.1, 0.5, 0.01),
    openai("gpt-5.6-sol", 4.0, 20.0, 0.4),
    openai("gpt-5.6-terra", 2.0, 12.0, 0.2),
    openai("gpt-5.6-luna", 0.2, 1.2, 0.02),
    openai("gpt-5.5", 5.0, 30.0, 0.5),
    openai("gpt-5.4", 2.5, 15.0, 0.25),
    openai("gpt-5.4-mini", 0.75, 4.5, 0.075),
    openai("gpt-5.4-nano", 0.2, 1.25, 0.02),
    openai("gpt-5.3-codex", 1.75, 14.0, 0.175),
    openai("gpt-5.2", 1.75, 14.0, 0.175),
    openai("gpt-5.1", 1.25, 10.0, 0.125),
    openai("gpt-5.1-codex", 1.25, 10.0, 0.125),
    openai("gpt-5", 1.25, 10.0, 0.125),
    openai("gpt-5-codex", 1.25, 10.0, 0.125),
    openai("gpt-5-mini", 0.25, 2.0, 0.025),
    openai("gpt-5-nano", 0.05, 0.4, 0.005),
    openai("o3", 2.0, 8.0, 0.5),
    openai("o4-mini", 1.1, 4.4, 0.275),
];

const MILLION: f64 = 1_000_000.0;

/// `None` for a model not in the table; a dated name (`-20250929`, `-2025-08-07`) takes its undated price.
#[must_use]
pub fn estimate(model: &str, tokens: TokenSums) -> Option<f64> {
    let base = undated(model);
    let price = PRICES.iter().find(|price| price.model == base)?;
    let cost = tokens.input * price.input
        + tokens.output * price.output
        + tokens.cache_read * price.cache_read
        + tokens.cache_write * price.cache_write;
    Some(cost / MILLION)
}

fn undated(model: &str) -> &str {
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    if let Some((base, date)) = model.rsplit_once('-')
        && date.len() == 8
        && digits(date)
    {
        return base;
    }
    let parts: Vec<&str> = model.rsplitn(4, '-').collect();
    if let [day, month, year, base] = parts[..]
        && year.len() == 4
        && month.len() == 2
        && day.len() == 2
        && [year, month, day].into_iter().all(digits)
    {
        return base;
    }
    model
}
