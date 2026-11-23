//! A cut of the hub's flow-test browser, which cannot be included by path: it stands on the hub suite's kit.

use std::collections::BTreeMap;

use reqwest::StatusCode;
use reqwest::redirect::Policy;

pub struct Answer {
    pub status: StatusCode,
    pub location: Option<String>,
    pub body: String,
}

pub struct Browser {
    client: reqwest::Client,
    base: String,
    cookies: BTreeMap<String, String>,
}

impl Browser {
    pub fn at(base: &str) -> Self {
        Self {
            client: reqwest::Client::builder()
                .redirect(Policy::none())
                .no_proxy()
                .build()
                .expect("a client"),
            base: base.to_owned(),
            cookies: BTreeMap::new(),
        }
    }

    async fn get(&mut self, path: &str) -> Answer {
        let mut request = self.client.get(format!("{}{path}", self.base));
        if !self.cookies.is_empty() {
            let pairs: Vec<String> = self
                .cookies
                .iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect();
            request = request.header("cookie", pairs.join("; "));
        }
        let response = request.send().await.expect("the hub answers");
        let status = response.status();
        let location = response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        for set in response.headers().get_all("set-cookie") {
            let text = set.to_str().expect("a cookie");
            let (pair, attributes) = text.split_once(';').unwrap_or((text, ""));
            let (name, value) = pair.split_once('=').expect("name=value");
            if value.is_empty() || attributes.contains("Max-Age=0") {
                self.cookies.remove(name);
            } else {
                self.cookies.insert(name.to_owned(), value.to_owned());
            }
        }
        let body = response.text().await.expect("a body");
        Answer {
            status,
            location,
            body,
        }
    }

    /// The link names the hub's public origin; only its path and query go to `base`.
    pub async fn open(&mut self, link: &str) -> String {
        let at = link.find("/connect/github").expect("a link to the hub");
        let bound = self.get(&link[at..]).await;
        assert_eq!(bound.status, StatusCode::SEE_OTHER, "{}", bound.body);
        assert_eq!(bound.location.as_deref(), Some("/connect/github/start"));
        let page = self.get("/connect/github/start").await;
        assert_eq!(page.status, StatusCode::OK, "{}", page.body);
        let action = page
            .body
            .split("action=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("a form");
        assert!(action.starts_with("https://github.com/settings/apps/new?state="));
        action.split("?state=").nth(1).expect("a state").to_owned()
    }

    pub async fn created(&mut self, code: &str, state: &str) -> Answer {
        self.get(&format!(
            "/connect/github/created?code={code}&state={state}"
        ))
        .await
    }
}
