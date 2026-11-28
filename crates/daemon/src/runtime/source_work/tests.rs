use super::*;
use crate::{
    config::origin::Origin,
    hub::{HubClient, TIMEOUT},
    token_store::{Credential, TokenStore},
};
use daemon_core::wire::{DaemonToken, WorkJob, WorkReport, WorkState};
use serde_json::json;
use std::sync::Arc;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
};

#[tokio::test]
#[ignore = "requires a disposable Linux container without a source-job broker"]
async fn failed_delivery_replays_and_hub_stop_is_durable_before_root_contact() {
    assert!(!std::path::Path::new(daemon_core::source_jobs::SOCKET).exists());
    let temporary = tempfile::tempdir().unwrap();
    let home = Home::at(temporary.path().join("daemon"));
    let device = Uuid::now_v7();
    let origin = Origin::parse("https://example.invalid").unwrap();
    let mut owner = Coordinator::open(&home, device, &origin).unwrap();
    let job = job();
    assert!(owner.register(&job).await.is_err());
    let execution = owner.executions().unwrap()[0];
    let report = WorkReport {
        id: job.id,
        attempt: job.attempt,
        state: WorkState::Preparing,
        log: None,
        result: None,
    };
    owner.report(execution, report.clone()).unwrap();
    let tokens = TokenStore::open(&home);
    tokens
        .save_new(
            &Credential {
                token: DaemonToken::from_random([42; 32]),
                expires_at: Utc::now() + chrono::TimeDelta::hours(1),
                hub: origin.clone(),
            },
            device,
        )
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap())
        .parse()
        .unwrap();
    let target = HubTarget {
        hub: Arc::new(HubClient::new(url, TIMEOUT).unwrap()),
        device,
        origin: origin.clone(),
        tokens,
    };
    let server = tokio::spawn(respond(listener, serde_json::to_value(&report).unwrap()));
    assert!(deliver(&mut owner, &target, execution).await.is_none());
    assert_eq!(owner.pending().unwrap(), vec![(execution, report)]);
    assert!(deliver(&mut owner, &target, execution).await.is_none());
    assert_eq!(owner.pending().unwrap(), []);
    drop(owner);
    let mut owner = Coordinator::open(&home, device, &origin).unwrap();
    assert!(matches!(
        owner.start(execution).await,
        Err(crate::source_work::Error::Conflict)
    ));
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap();
}

async fn respond(listener: TcpListener, expected: serde_json::Value) {
    for (status, body) in [
        (503, json!({"error":"UNAVAILABLE"})),
        (
            200,
            json!({"lease":{"until":(Utc::now()+chrono::TimeDelta::seconds(60)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)},
            "stop":{"reason":"operator"},"logEnd":0}),
        ),
    ] {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut reader = BufReader::new(&mut socket);
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        assert_eq!(line.trim(), "POST /work/v1/report HTTP/1.1");
        let mut length = None;
        loop {
            line.clear();
            reader.read_line(&mut line).await.unwrap();
            if line == "\r\n" {
                break;
            }
            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                length = Some(value.trim().parse::<usize>().unwrap());
            }
        }
        let mut bytes = vec![0; length.unwrap()];
        reader.read_exact(&mut bytes).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
            expected
        );
        drop(reader);
        let body = serde_json::to_vec(&body).unwrap();
        let headers = format!(
            "HTTP/1.1 {status} Response\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        socket.write_all(headers.as_bytes()).await.unwrap();
        socket.write_all(&body).await.unwrap();
    }
}

fn job() -> WorkJob {
    serde_json::from_value(json!({
        "id":Uuid::now_v7(),"type":"build","kind":"container","attempt":1,
        "lease":{"until":(Utc::now()+chrono::TimeDelta::seconds(60)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)},
        "limits":{"seconds":300,"idleSeconds":60,"memoryBytes":536_870_912,
            "diskBytes":22_548_578_304_u64,"orphanSeconds":60},
        "project":null,"repository":{"forge":"github","owner":"example","name":"sample"},
        "secrets":[],"parameters":{"recipe":"source_image_v1","commit":"a".repeat(40),
            "target":"linux-aarch64","strategy":"auto","trigger":"operator","origin":"default_branch"}
    })).unwrap()
}
