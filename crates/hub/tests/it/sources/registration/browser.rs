//! A stand-in for the operator's browser that plays GitHub's redirects itself.

use std::{collections::BTreeMap, net::SocketAddr, sync::Arc, time::Duration};

use anima_hub::{
    all_routes,
    clock::{Clock, FixedClock},
    config::Config,
    engine::sources::{Attempts, Reach, Registry},
    fleet::{access, events::Bus},
    schema::Dependencies,
};
use reqwest::{StatusCode, header::HeaderMap, redirect::Policy};
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::task::JoinHandle;

use crate::{
    hub::{Hub, dependencies, set_operator, start},
    sources::{Kit, TEST_KEY, intake, stub::Stub},
};

pub const CODE: &str = "CODEMARKER7c1f";
pub const CONVERSION: &str = "/app-manifests/CODEMARKER7c1f/conversions";
pub const INSTALLATION: &str = "4321";
pub const PUBLIC: &str = "https://hub.anima.ing";
pub const LIST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../providers/github/tests/fixtures/installations.json"
));
pub const ONE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../providers/github/tests/fixtures/installation.json"
));

/// Holds two secrets that the hub never reads.
pub fn converted() -> String {
    json!({
        "id": 1_234_567, "slug": "anima-k3f9x2mq7b",
        "owner": { "login": "octo-person", "id": 9001, "type": "User" },
        "client_secret": "CLIENT-SECRET-MARKER-0000",
        "webhook_secret": "WEBHOOK-SECRET-MARKER-1111",
        "pem": *TEST_KEY,
    })
    .to_string()
}

pub struct Served {
    pub kit: Kit,
    pub base: String,
    pub origin: String,
    task: JoinHandle<()>,
}

impl Drop for Served {
    fn drop(&mut self) {
        self.task.abort();
    }
}

const PAGE_WAIT: Duration = Duration::from_millis(500);

pub async fn served(pool: &PgPool, poll_every: Duration) -> Served {
    served_waiting(pool, poll_every, PAGE_WAIT).await
}

pub async fn served_waiting(pool: &PgPool, poll_every: Duration, page_wait: Duration) -> Served {
    let config = anima_hub::config::from_env(&BTreeMap::new()).unwrap();
    serve(pool, poll_every, page_wait, config).await
}

pub async fn served_as(pool: &PgPool, poll_every: Duration, config: Config) -> Served {
    serve(pool, poll_every, PAGE_WAIT, config).await
}

async fn serve(pool: &PgPool, poll_every: Duration, page_wait: Duration, config: Config) -> Served {
    let origin = config.public_origin.clone();
    let config = Arc::new(config);
    let stub = Stub::start(pool).await;
    let transport = anima_hub::engine::sources::Transport::to_stub(
        stub.base.clone(),
        anima_hub::fleet::targets::AddressPolicy::default().allowing_loopback(),
    )
    .unwrap();
    intake::make_table(pool).await;
    set_operator(pool).await;
    let networks: [ipnet::IpNet; 1] = ["127.0.0.0/8".parse().unwrap()];
    access::permit_networks(pool, &networks).await.unwrap();
    let intake = intake::StubIntake::default();
    let registry = Registry::new(
        vec![provider_github::DECLARATION],
        vec![Arc::new(intake.clone())],
    );
    let reach = Reach {
        attempts: Attempts::new(poll_every, page_wait),
        ..Reach::new(registry, transport)
    };
    let clock: Arc<dyn Clock> = Arc::new(FixedClock(start()));
    let parts = || Dependencies {
        config: config.clone(),
        sources: reach.clone(),
        ..dependencies(pool, clock.clone(), Bus::new(16))
    };
    // A hosted hub refuses the password sign-in (HMD-037), so a default hub signs in.
    let token = Hub::serving(pool, dependencies(pool, clock.clone(), Bus::new(4)))
        .token()
        .await;
    let hub = Hub::serving(pool, parts());
    let edge = access::Edge::new(pool.clone(), Bus::new(4), clock.clone());
    let app = all_routes(parts(), edge);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let service = app.into_make_service_with_connect_info::<SocketAddr>();
    let task = tokio::spawn(async move { axum::serve(listener, service).await.unwrap() });
    stub.reply(CONVERSION, reply(201, &converted()));
    stub.reply(
        &format!("/app/installations/{INSTALLATION}"),
        reply(200, ONE),
    );
    let kit = Kit {
        pool: pool.clone(),
        hub,
        token,
        stub,
        intake,
        reach,
    };
    Served {
        kit,
        base: format!("http://{addr}"),
        origin,
        task,
    }
}

pub fn reply(status: u16, body: &str) -> crate::sources::stub::Reply {
    crate::sources::stub::Reply {
        status,
        headers: Vec::new(),
        body: body.to_owned(),
    }
}

pub struct Answer {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: String,
}

impl Answer {
    pub fn location(&self) -> &str {
        self.headers
            .get("location")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_else(|| panic!("no Location: {} {}", self.status, self.body))
    }
}

pub struct Browser {
    client: reqwest::Client,
    base: String,
    origin: String,
    pub cookies: BTreeMap<String, String>,
}

impl Browser {
    pub fn new(served: &Served) -> Self {
        Self::with(served, HeaderMap::new())
    }

    /// Passes the name `host`, as the pool's gateway passes the name the browser used (HMD-061).
    pub fn named(served: &Served, host: &str) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert("host", host.parse().unwrap());
        Self::with(served, headers)
    }

    fn with(served: &Served, headers: HeaderMap) -> Self {
        Self {
            client: reqwest::Client::builder()
                .redirect(Policy::none())
                .default_headers(headers)
                .build()
                .unwrap(),
            base: served.base.clone(),
            origin: served.origin.clone(),
            cookies: BTreeMap::new(),
        }
    }

    fn cookie_header(&self) -> String {
        let pairs: Vec<String> = self
            .cookies
            .iter()
            .map(|(n, v)| format!("{n}={v}"))
            .collect();
        pairs.join("; ")
    }

    async fn send(&mut self, request: reqwest::RequestBuilder) -> Answer {
        let request = if self.cookies.is_empty() {
            request
        } else {
            request.header("cookie", self.cookie_header())
        };
        let response = request.send().await.unwrap();
        let (status, headers) = (response.status(), response.headers().clone());
        for set in headers.get_all("set-cookie") {
            let text = set.to_str().unwrap();
            let (pair, attributes) = text.split_once(';').unwrap_or((text, ""));
            let (name, value) = pair.split_once('=').unwrap();
            if value.is_empty() || attributes.contains("Max-Age=0") {
                self.cookies.remove(name);
            } else {
                self.cookies.insert(name.to_owned(), value.to_owned());
            }
        }
        let body = response.text().await.unwrap();
        Answer {
            status,
            headers,
            body,
        }
    }

    pub async fn get(&mut self, path: &str) -> Answer {
        let request = self.client.get(format!("{}{path}", self.base));
        self.send(request).await
    }

    pub async fn post(&mut self, path: &str, headers: &[(&str, &str)]) -> Answer {
        let mut request = self
            .client
            .post(format!("{}{path}", self.base))
            .header("content-type", "application/x-www-form-urlencoded");
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        self.send(request).await
    }

    pub async fn sign_in(&mut self) {
        let body = json!({
            "query": "mutation($input: LoginInput!) { signInBrowser(input: $input) { expiresAt } }",
            "variables": { "input": { "username": crate::hub::USERNAME, "password": crate::hub::PASSWORD } },
        });
        let request = self
            .client
            .post(format!("{}/graphql", self.base))
            .header("content-type", "application/json")
            .header("origin", self.origin.as_str())
            .body(body.to_string());
        let answer = self.send(request).await;
        assert!(self.cookies.contains_key("__Host-anima"), "{}", answer.body);
    }

    pub async fn open(&mut self, link: &str) -> String {
        let path = link.strip_prefix(&self.origin).expect("a link to the hub");
        let bound = self.get(path).await;
        assert_eq!(bound.status, StatusCode::SEE_OTHER, "{}", bound.body);
        assert_eq!(bound.location(), "/connect/github/start");
        self.form_state().await
    }

    pub async fn form_state(&mut self) -> String {
        let page = self.get("/connect/github/start").await;
        assert_eq!(page.status, StatusCode::OK, "{}", page.body);
        let action = page
            .body
            .split("action=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .unwrap();
        let state = action
            .split("?state=")
            .nth(1)
            .expect("a state in the action");
        assert!(action.starts_with("https://github.com/settings/apps/new?state="));
        state.to_owned()
    }

    pub async fn created(&mut self, state: &str) -> Answer {
        self.get(&format!(
            "/connect/github/created?code={CODE}&state={state}"
        ))
        .await
    }

    pub async fn installed(&mut self, installation: &str, state: &str) -> Answer {
        let path = format!(
            "/connect/github/installed?installation_id={installation}&setup_action=install&state={state}"
        );
        self.get(&path).await
    }
}

pub fn install_state(answer: &Answer) -> String {
    assert_eq!(answer.status, StatusCode::SEE_OTHER, "{}", answer.body);
    let location = answer.location();
    let state = location
        .strip_prefix("https://github.com/apps/anima-k3f9x2mq7b/installations/new?state=")
        .unwrap_or_else(|| panic!("not the install page: {location}"));
    state.to_owned()
}

pub async fn shown(kit: &Kit) -> Value {
    let response = kit
        .hub
        .post(Some(&kit.token), super::STATE, json!({}))
        .await;
    response["data"]["githubConnect"].clone()
}

pub fn conversions(kit: &Kit) -> usize {
    kit.stub
        .paths()
        .iter()
        .filter(|path| path.starts_with("/app-manifests/"))
        .count()
}
