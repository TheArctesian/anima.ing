//! A native forwarding credential stays within the desktop transport.
use agent_threads::managed::preview::{FORWARD_CHUNK, FORWARD_PROTOCOL, ForwardSession};
use futures_util::{SinkExt as _, StreamExt as _};
use std::{io, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    task::JoinSet,
    time::{Instant, timeout},
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream,
    tungstenite::{Bytes, Message, client::IntoClientRequest as _, protocol::WebSocketConfig},
};
use zeroize::Zeroizing;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub(super) struct Connection {
    client: reqwest::Client,
    https: url::Url,
    wss: url::Url,
    credential: Zeroizing<String>,
    expires_at: Instant,
}

pub(super) fn valid_secret(token: &str) -> bool {
    token.len() == 43
        && token
            .bytes()
            .all(|part| part.is_ascii_alphanumeric() || b"-_".contains(&part))
}

impl Connection {
    pub(super) async fn redeem(https: url::Url, token: &str) -> Result<Self, &'static str> {
        let client = reqwest::Client::builder()
            .https_only(true)
            .http1_only()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| "forward_unreachable")?;
        let mut answer = client
            .post(https.clone())
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .body(token.to_owned())
            .send()
            .await
            .map_err(|_| "forward_unreachable")?;
        if answer.status() != reqwest::StatusCode::OK {
            return Err("forward_refused");
        }
        let mut body = Zeroizing::new(Vec::new());
        while let Some(chunk) = answer.chunk().await.map_err(|_| "forward_unreachable")? {
            if body.len() + chunk.len() > 256 {
                return Err("forward_refused");
            }
            body.extend_from_slice(&chunk);
        }
        let ForwardSession { token, seconds } =
            serde_json::from_slice(&body).map_err(|_| "forward_refused")?;
        let credential = Zeroizing::new(token);
        if !valid_secret(&credential) || !(1..=600).contains(&seconds) {
            return Err("forward_refused");
        }
        let mut wss = https.clone();
        wss.set_scheme("wss").map_err(|()| "invalid_forward")?;
        Ok(Self {
            client,
            https,
            wss,
            credential,
            expires_at: Instant::now() + Duration::from_secs(seconds),
        })
    }

    pub(super) async fn serve(self, listener: TcpListener) {
        let shared = Arc::new(self);
        tokio::select! {
            () = shared.watch() => (),
            () = shared.accept(listener) => (),
        }
    }

    async fn watch(&self) {
        let mut ticks = tokio::time::interval(Duration::from_secs(5));
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                () = tokio::time::sleep_until(self.expires_at) => return,
                _ = ticks.tick() => (),
            }
            let checking = self
                .client
                .head(self.https.clone())
                .bearer_auth(self.credential.as_str())
                .send();
            let answer = tokio::select! {
                () = tokio::time::sleep_until(self.expires_at) => return,
                answer = checking => answer,
            };
            if !answer.is_ok_and(|answer| answer.status() == reqwest::StatusCode::NO_CONTENT) {
                return;
            }
        }
    }

    async fn accept(self: &Arc<Self>, listener: TcpListener) {
        let mut peers = JoinSet::new();
        loop {
            tokio::select! {
                _ = peers.join_next(), if !peers.is_empty() => (),
                accepted = listener.accept() => {
                    let Ok((client, _)) = accepted else { return };
                    if peers.len() < 16 {
                        let target = Arc::clone(self);
                        peers.spawn(async move {
                            if let Ok(Ok(port)) = timeout(Duration::from_secs(20), target.connect()).await {
                                let _ = relay(client, port).await;
                            }
                        });
                    }
                }
            }
        }
    }

    async fn connect(&self) -> Result<Socket, ()> {
        use tokio_tungstenite::tungstenite::http::{HeaderValue, header};
        let mut request = self.wss.as_str().into_client_request().map_err(|_| ())?;
        let value = Zeroizing::new(format!("Bearer {}", self.credential.as_str()));
        let mut authorization = HeaderValue::from_str(&value).map_err(|_| ())?;
        authorization.set_sensitive(true);
        request
            .headers_mut()
            .insert(header::AUTHORIZATION, authorization);
        request.headers_mut().insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static(FORWARD_PROTOCOL),
        );
        let config = WebSocketConfig::default()
            .read_buffer_size(FORWARD_CHUNK)
            .write_buffer_size(FORWARD_CHUNK)
            .max_write_buffer_size(3 * FORWARD_CHUNK)
            .max_frame_size(Some(FORWARD_CHUNK))
            .max_message_size(Some(FORWARD_CHUNK));
        let (socket, answer) =
            tokio_tungstenite::connect_async_with_config(request, Some(config), false)
                .await
                .map_err(|_| ())?;
        if answer
            .headers()
            .get(header::SEC_WEBSOCKET_PROTOCOL)
            .is_none_or(|value| value != FORWARD_PROTOCOL)
        {
            return Err(());
        }
        Ok(socket)
    }
}

async fn relay(client: TcpStream, remote: Socket) -> io::Result<()> {
    let broken = |_| io::Error::from(io::ErrorKind::ConnectionAborted);
    let (mut send, mut receive) = remote.split();
    let (mut read, mut write) = client.into_split();
    let upload = async {
        let mut chunk = vec![0; FORWARD_CHUNK];
        loop {
            let count = read.read(&mut chunk).await?;
            send.send(Message::Binary(Bytes::copy_from_slice(&chunk[..count])))
                .await
                .map_err(broken)?;
            if count == 0 {
                return Ok(());
            }
        }
    };
    let download = async {
        while let Some(frame) = receive.next().await {
            match frame.map_err(broken)? {
                Message::Binary(bytes) if bytes.is_empty() => return write.shutdown().await,
                Message::Binary(bytes) => write.write_all(&bytes).await?,
                Message::Ping(_) | Message::Pong(_) => (),
                _ => break,
            }
        }
        Err(io::ErrorKind::ConnectionAborted.into())
    };
    tokio::try_join!(upload, download)?;
    Ok(())
}
