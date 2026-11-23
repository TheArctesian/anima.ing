use chrono::{DateTime, SecondsFormat, Utc};

use super::server::{Server, reply, serve};
use crate::fleet::{checks::probe::rdap::Rdap, targets::AddressPolicy};

pub(in crate::fleet::checks) async fn registry(
    answer: impl Fn(&str) -> Vec<u8> + Send + Sync + 'static,
) -> Server {
    serve(move |path, port| {
        if path == "/dns.json" {
            let file =
                format!(r#"{{"services": [[["test"], ["https://localhost:{port}/rdap/"]]]}}"#);
            reply("200 OK", file.as_bytes())
        } else {
            answer(path.trim_start_matches("/rdap/domain/"))
        }
    })
    .await
}

pub(in crate::fleet::checks) fn expiring_at(
    not_after: DateTime<Utc>,
) -> impl Fn(&str) -> Vec<u8> + Send + Sync + 'static {
    let date = not_after.to_rfc3339_opts(SecondsFormat::Secs, true);
    move |_| {
        let body =
            format!(r#"{{"events": [{{"eventAction": "expiration", "eventDate": "{date}"}}]}}"#);
        reply("200 OK", body.as_bytes())
    }
}

pub(in crate::fleet::checks) fn rdap(server: &Server) -> Rdap {
    Rdap::for_tests(
        AddressPolicy::default().allowing_loopback(),
        server.root.clone(),
        server.url("/dns.json"),
    )
}
