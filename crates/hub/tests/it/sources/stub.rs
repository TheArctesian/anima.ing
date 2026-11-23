//! A loopback stand-in for GitHub; tests never reach the real GitHub (LOG-007, PRJ-018).

use std::{
    collections::{HashMap, HashSet, VecDeque},
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Instant,
};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, Method, StatusCode, Uri, header},
    response::Response,
};
use reqwest::Url;
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::sync::Semaphore;
use uuid::Uuid;

#[derive(Clone)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(&'static str, String)>,
    pub body: String,
}

#[derive(Clone)]
pub struct Heard {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, Vec<u8>)>,
    pub body: Vec<u8>,
    pub at: Instant,
}

impl Heard {
    pub fn header(&self, name: &str) -> Vec<&[u8]> {
        self.headers
            .iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_slice())
            .collect()
    }
}

pub struct Script {
    /// `(method, path with query, sent a bearer token)` of each request, in order.
    pub seen: Vec<(String, String, bool)>,
    pub heard: Vec<Heard>,
    pub app_tokens: Vec<String>,
    pub mint_status: u16,
    pub token: String,
    pub pages: Vec<Value>,
    pub replies: HashMap<String, Reply>,
    /// SQL run once when its path is first requested, as another writer would.
    pub hooks: HashMap<String, String>,
    pub endless: HashSet<String>,
    pub gates: HashMap<String, Arc<Semaphore>>,
    pub dropped: HashSet<String>,
    pub queued: HashMap<String, VecDeque<Reply>>,
}

#[derive(Clone)]
pub struct Stub {
    pub script: Arc<Mutex<Script>>,
    pool: PgPool,
    pub base: Url,
}

impl Stub {
    pub async fn start(pool: &PgPool) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address: SocketAddr = listener.local_addr().unwrap();
        let script = Script {
            seen: Vec::new(),
            heard: Vec::new(),
            app_tokens: Vec::new(),
            mint_status: 201,
            // A shape of its own, so the hub cannot rely on a GitHub token length or prefix.
            token: format!("stubtoken-{}", Uuid::now_v7().simple()),
            pages: Vec::new(),
            replies: HashMap::new(),
            hooks: HashMap::new(),
            endless: HashSet::new(),
            gates: HashMap::new(),
            dropped: HashSet::new(),
            queued: HashMap::new(),
        };
        let stub = Self {
            script: Arc::new(Mutex::new(script)),
            pool: pool.clone(),
            base: Url::parse(&format!("http://{address}")).unwrap(),
        };
        let app = Router::new().fallback(answer).with_state(stub.clone());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        stub
    }

    pub fn with<T>(&self, f: impl FnOnce(&mut Script) -> T) -> T {
        f(&mut self.script.lock().unwrap())
    }

    pub fn paths(&self) -> Vec<String> {
        self.with(|s| s.seen.iter().map(|(_, path, _)| path.clone()).collect())
    }

    pub fn gets(&self) -> Vec<String> {
        self.with(|s| {
            s.seen
                .iter()
                .filter(|(method, _, _)| method == "GET")
                .map(|(_, path, _)| path.clone())
                .collect()
        })
    }

    pub fn token(&self) -> String {
        self.with(|s| s.token.clone())
    }

    pub fn set_pages(&self, pages: &[&[(&str, &str)]]) {
        let pages = pages
            .iter()
            .map(|items| {
                Value::Array(
                    items
                        .iter()
                        .map(|(id, value)| json!({ "id": id, "value": value }))
                        .collect(),
                )
            })
            .collect();
        self.with(|s| s.pages = pages);
    }

    pub fn reply(&self, path: &str, reply: Reply) {
        self.with(|s| s.replies.insert(path.to_owned(), reply));
    }

    pub fn reply_to(&self, method: &str, path: &str, reply: Reply) {
        self.with(|s| s.replies.insert(format!("{method} {path}"), reply));
    }

    pub fn reply_once(&self, method: &str, path: &str, reply: Reply) {
        self.with(|s| {
            let queue = s.queued.entry(format!("{method} {path}")).or_default();
            queue.push_back(reply);
        });
    }

    pub fn drop_answer(&self, path: &str) {
        self.with(|s| s.dropped.insert(path.to_owned()));
    }

    /// Only a reader that stops at its cap returns.
    pub fn endless(&self, path: &str) {
        self.with(|s| s.endless.insert(path.to_owned()));
    }

    pub fn gate(&self, path: &str, gate: &Arc<Semaphore>) {
        self.with(|s| s.gates.insert(path.to_owned(), gate.clone()));
    }

    pub fn hook(&self, path: &str, sql: &str) {
        self.with(|s| s.hooks.insert(path.to_owned(), sql.to_owned()));
    }
}

fn respond(status: u16, headers: &[(&'static str, String)], body: String) -> Response {
    let mut response = Response::builder().status(StatusCode::from_u16(status).unwrap());
    for (name, value) in headers {
        response = response.header(*name, value);
    }
    response
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap()
}

fn endless() -> Response {
    let chunk = Bytes::from(vec![b' '; 64 * 1024]);
    let chunks = futures_util::stream::repeat_with(move || Ok::<_, std::io::Error>(chunk.clone()));
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from_stream(chunks))
        .unwrap()
}

fn dropped() -> Response {
    let chunks = futures_util::stream::iter([
        Ok::<_, std::io::Error>(Bytes::from_static(b"{\"partial\":")),
        Err(std::io::Error::other("dropped")),
    ]);
    Response::builder()
        .status(StatusCode::CREATED)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from_stream(chunks))
        .unwrap()
}

fn hear(
    stub: &Stub,
    (method, uri, headers): (&Method, &Uri, &HeaderMap),
    body: &Bytes,
) -> Option<String> {
    let path = uri.path_and_query().map_or("/", |p| p.as_str()).to_owned();
    let bearer = headers
        .get(header::AUTHORIZATION)
        .is_some_and(|v| v.as_bytes().starts_with(b"Bearer "));
    let app_token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|_| method == Method::POST)
        .map(str::to_owned);
    stub.with(|s| {
        s.seen.push((method.to_string(), path.clone(), bearer));
        s.heard.push(Heard {
            method: method.to_string(),
            path,
            headers: headers
                .iter()
                .map(|(n, v)| (n.as_str().to_owned(), v.as_bytes().to_vec()))
                .collect(),
            body: body.to_vec(),
            at: Instant::now(),
        });
        s.app_tokens.extend(app_token);
        s.hooks.remove(uri.path())
    })
}

fn scripted_reply(script: &mut Script, method: &Method, uri: &Uri) -> Option<Reply> {
    let key = format!("{method} {}", uri.path());
    script
        .queued
        .get_mut(&key)
        .and_then(VecDeque::pop_front)
        .or_else(|| {
            script
                .replies
                .get(&key)
                .or_else(|| {
                    uri.path_and_query()
                        .and_then(|path| script.replies.get(path.as_str()))
                })
                .or_else(|| script.replies.get(uri.path()))
                .cloned()
        })
}

async fn answer(
    State(stub): State<Stub>,
    (method, uri, headers): (Method, Uri, HeaderMap),
    body: Bytes,
) -> Response {
    if let Some(sql) = hear(&stub, (&method, &uri, &headers), &body) {
        sqlx::query(&sql).execute(&stub.pool).await.unwrap();
    }
    if let Some(gate) = stub.with(|s| s.gates.get(uri.path()).cloned()) {
        gate.acquire().await.unwrap().forget();
    }
    if stub.with(|s| s.endless.contains(uri.path())) {
        return endless();
    }
    if stub.with(|s| s.dropped.contains(uri.path())) {
        return dropped();
    }
    let (reply, mint_status, token, pages) = stub.with(|s| {
        (
            scripted_reply(s, &method, &uri),
            s.mint_status,
            s.token.clone(),
            s.pages.clone(),
        )
    });
    if let Some(reply) = reply {
        return respond(reply.status, &reply.headers, reply.body);
    }
    if method == Method::POST && uri.path().starts_with("/app/installations/") {
        let body = json!({ "token": token, "expires_at": "2099-01-01T00:00:00Z" });
        return respond(mint_status, &[], body.to_string());
    }
    if method == Method::GET
        && uri.path().ends_with("/commits")
        && uri
            .query()
            .is_some_and(|query| query.split('&').any(|pair| pair == "per_page=1"))
    {
        return respond(409, &[], "{}".to_owned());
    }
    let page: Option<usize> = uri
        .path()
        .strip_prefix("/pages/")
        .and_then(|n| n.parse().ok());
    match page.and_then(|n| pages.get(n - 1).map(|items| (n, items))) {
        Some((n, items)) => {
            let next = (n < pages.len()).then_some(n + 1);
            respond(
                200,
                &[],
                json!({ "items": items, "next": next }).to_string(),
            )
        }
        None => respond(404, &[], "{}".to_owned()),
    }
}
