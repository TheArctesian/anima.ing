use super::*;

const START: &str = concat!(
    "{\"event\":\"cmd_name\",\"sid\":\"git-P00000012\",\"name\":\"clone\"}\n",
    "{\"event\":\"child_start\",\"sid\":\"git-P00000012\",\"child_id\":0,\"child_class\":\"remote-https\",\"use_shell\":false}\n",
    "{\"event\":\"cmd_name\",\"sid\":\"git-P00000012/http-P00000013\",\"name\":\"remote-curl\"}\n",
);
const NEGOTIATION: &str = "{\"event\":\"data\",\"sid\":\"git-P00000012/http-P00000013\",\"category\":\"transfer\",\"key\":\"negotiated-version\",\"value\":\"2\"}\n";
const END: &str = concat!(
    "{\"event\":\"exit\",\"sid\":\"git-P00000012/http-P00000013\",\"code\":0}\n",
    "{\"event\":\"child_exit\",\"sid\":\"git-P00000012\",\"child_id\":0,\"pid\":19,\"code\":0}\n",
    "{\"event\":\"exit\",\"sid\":\"git-P00000012\",\"code\":0}\n",
);

fn verify(event: &str, end: &str) -> bool {
    let trace = format!("{START}{event}{end}");
    negotiated(trace.as_bytes(), 18, "remote-https")
}

#[test]
fn only_the_launched_http_helper_can_supply_negotiation_evidence() {
    assert!(verify(NEGOTIATION, END));
    assert!(!verify(&NEGOTIATION.replace("00000013", "00000014"), END));
    assert!(!verify(&format!("remote: {NEGOTIATION}"), END));
    assert!(!verify(&NEGOTIATION.repeat(2), END));
    assert!(!verify(
        NEGOTIATION,
        &END.replace("\"pid\":19", "\"pid\":20")
    ));
}

#[test]
fn malformed_truncated_or_overlong_traces_are_refused() {
    let valid = format!("{START}{NEGOTIATION}{END}");
    assert!(!negotiated(valid.trim_end().as_bytes(), 18, "remote-https"));
    let malformed = format!("{valid}{{\"event\":\n");
    assert!(!negotiated(malformed.as_bytes(), 18, "remote-https"));
    let overlong = format!("{valid}{}", "warning: fixture\n".repeat(MAX_EVENTS));
    assert!(!negotiated(overlong.as_bytes(), 18, "remote-https"));
}
