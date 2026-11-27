//! One provider-specific human response on an already authorized live stream.
use super::{Error, Result};
use agent_threads::managed::provider_login::{Provider, valid_code};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use zeroize::Zeroizing;

#[cfg(test)]
#[path = "provider_reply_tests.rs"]
mod tests;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    code: String,
}

pub(super) async fn forward(
    provider: Provider,
    input: impl AsyncRead + Unpin,
    output: &mut (impl AsyncWrite + Unpin),
) -> Result<()> {
    let mut input = BufReader::new(input);
    if matches!(provider, Provider::Claude) {
        let mut frame = Zeroizing::new(Vec::new());
        (&mut input)
            .take(2049)
            .read_until(b'\n', &mut frame)
            .await?;
        if frame.len() > 2048 || frame.last() != Some(&b'\n') {
            return Err(Error::Refused("Invalid provider response."));
        }
        let reply: Reply = serde_json::from_slice(&frame)
            .map_err(|_| Error::Refused("Invalid provider response."))?;
        let code = Zeroizing::new(reply.code);
        if !valid_code(&code) {
            return Err(Error::Refused("Invalid provider response."));
        }
        output.write_all(&frame).await?;
        output.flush().await?;
    }
    // Codex takes no reply. Claude takes exactly one; more bytes or EOF cancel it.
    let _ = input.read(&mut [0_u8; 1]).await?;
    Ok(())
}
