use super::*;

pub(super) async fn poll(State(hub): State<Hub>) -> Response {
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut state = hub.0.lock().unwrap();
    if !state.done || state.delivered == 2 || state.delivered != state.receipts {
        return StatusCode::NO_CONTENT.into_response();
    }
    state.delivered += 1;
    Json(json!({"commands":[{
        "id":COMMAND, "account":ACCOUNT, "device":DEVICE, "launch":LAUNCH, "thread":state.runner,
        "payload":{"kind":"prompt", "prompt":"fixture"},
    }]}))
    .into_response()
}

pub(super) async fn receipt(State(hub): State<Hub>, Json(body): Json<Value>) -> StatusCode {
    assert_eq!(body["id"], COMMAND);
    assert_eq!(body["outcome"], "recorded");
    hub.0.lock().unwrap().receipts += 1;
    StatusCode::OK
}
