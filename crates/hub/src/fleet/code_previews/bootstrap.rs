//! Credential-free launch page; only a trusted app opener can deliver the one-use token.
use super::{Browser, CLIENT_ORIGINS, protect, refusal};
use axum::{
    http::{HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};

pub(super) fn page(state: &Browser) -> Response {
    let allowed = serde_json::json!([state.app_origin, CLIENT_ORIGINS[0], CLIENT_ORIGINS[1]]);
    let script = format!(
        r"const allowed={allowed};
const source=window.opener; let used=false;
const fail=()=>{{window.opener=null;document.body.textContent='Preview could not open. Return to Code and prepare it again.';}};
const timer=setTimeout(()=>{{used=true;fail();}},60000);
window.addEventListener('message',async event=>{{
 if(used||event.source!==source||!allowed.includes(event.origin)||event.data?.type!=='anima-preview-open'||typeof event.data.token!=='string'||!/^[A-Za-z0-9_-]{{43}}$/.test(event.data.token))return;
 used=true;clearTimeout(timer);window.opener=null;
 try{{const response=await fetch('/.anima/handoff',{{method:'POST',credentials:'same-origin',headers:{{'Content-Type':'application/x-www-form-urlencoded'}},body:'token='+event.data.token}});if(!response.ok){{fail();return;}}location.replace('/');}}catch{{fail();}}
}});
if(source)source.postMessage({{type:'anima-preview-ready'}},'*');"
    );
    let hash = STANDARD.encode(Sha256::digest(script.as_bytes()));
    let policy = format!(
        "default-src 'none'; script-src 'sha256-{hash}'; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'"
    );
    let Ok(policy) = HeaderValue::from_str(&policy) else {
        return refusal(StatusCode::INTERNAL_SERVER_ERROR);
    };
    let mut response = Html(format!("<!doctype html><meta charset=utf-8><title>Opening preview</title><body>Opening preview…<script>{script}</script></body>")).into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_SECURITY_POLICY, policy);
    protect(&mut response);
    response
}
