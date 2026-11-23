//! Library tests against Hugging Face answers recorded on 2026-11-20 in `tests/recorded/`.

mod aliases;
mod reads;
mod widening;

use std::sync::atomic::{AtomicUsize, Ordering};

use super::fetch::{Fetch, FetchError};

const GGUF_ANSWER: &[u8] = include_bytes!("tests/recorded/unsloth-qwen3-0.6b-gguf.json");
const GGUF_CONFIG: &[u8] = include_bytes!("tests/recorded/unsloth-qwen3-0.6b-gguf.config.json");
const MLX_ANSWER: &[u8] = include_bytes!("tests/recorded/mlx-qwen3-0.6b-4bit.json");
const MLX_CONFIG: &[u8] = include_bytes!("tests/recorded/mlx-qwen3-0.6b-4bit.config.json");

/// A small file that was not recorded answers fixed bytes; the tests check `config.json` only.
#[derive(Default)]
pub(super) struct Recorded {
    reads: AtomicUsize,
}

impl Recorded {
    pub(super) fn reads(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }
}

#[expect(
    clippy::unused_async_trait_impl,
    reason = "a recorded answer needs no wait"
)]
impl Fetch for Recorded {
    async fn repository(&self, repository: &str) -> Result<Vec<u8>, FetchError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        match repository {
            "unsloth/Qwen3-0.6B-GGUF" => Ok(GGUF_ANSWER.to_vec()),
            // The same files under an organisation that is not in the list.
            "someone/Qwen3-0.6B-GGUF" => Ok(String::from_utf8_lossy(GGUF_ANSWER)
                .replace("unsloth/Qwen3-0.6B-GGUF", repository)
                .into_bytes()),
            "mlx-community/Qwen3-0.6B-4bit" => Ok(MLX_ANSWER.to_vec()),
            _ => Err(FetchError::NotFound),
        }
    }

    async fn file(
        &self,
        repository: &str,
        _revision: &str,
        path: &str,
    ) -> Result<Vec<u8>, FetchError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        match (repository, path) {
            ("unsloth/Qwen3-0.6B-GGUF" | "someone/Qwen3-0.6B-GGUF", "config.json") => {
                Ok(GGUF_CONFIG.to_vec())
            }
            ("mlx-community/Qwen3-0.6B-4bit", "config.json") => Ok(MLX_CONFIG.to_vec()),
            ("mlx-community/Qwen3-0.6B-4bit", _) => Ok(b"not recorded".to_vec()),
            _ => Err(FetchError::NotFound),
        }
    }
}
