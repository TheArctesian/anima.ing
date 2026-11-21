//! The requests of the one interface (SF-231): none names a device or a port.

use tokio::io::BufReader;

use crate::wire::{Dial, Request, Transport, WireError, parse_request, read_line};

#[test]
fn a_connect_names_an_application_and_its_route_and_nothing_else() {
    let id = "0190a000-0000-7000-8009-000000000001";
    let line = format!(r#"{{"connect":{{"application":"{id}","name":"mail.example.com"}}}}"#);
    assert_eq!(
        parse_request(&line).unwrap(),
        Request::Connect(Dial {
            application: id.parse().unwrap(),
            name: "mail.example.com".to_owned(),
            transport: Transport::Tcp,
        })
    );
    let udp = format!(
        r#"{{"connect":{{"application":"{id}","name":"vpn.example.com","transport":"udp"}}}}"#
    );
    assert!(matches!(
        parse_request(&udp).unwrap(),
        Request::Connect(Dial {
            transport: Transport::Udp,
            ..
        })
    ));
    for refused in [
        format!(r#"{{"connect":{{"application":"{id}"}}}}"#),
        format!(r#"{{"connect":{{"application":"{id}","name":"a","port":22}}}}"#),
        r#"{"connect":{"device":"x","port":22}}"#.to_owned(),
        "dial fd7a:115c:a1e0::1 22".to_owned(),
    ] {
        assert!(
            matches!(parse_request(&refused), Err(WireError::Malformed)),
            "{refused}"
        );
    }
}

#[tokio::test]
async fn a_line_longer_than_its_limit_is_refused() {
    let long = format!("{}\n", "x".repeat(40));
    let mut reader = BufReader::new(long.as_bytes());
    assert!(matches!(
        read_line(&mut reader, 32).await,
        Err(WireError::TooLong)
    ));
    let mut reader = BufReader::new(&b"\"table\"\n"[..]);
    let line = read_line(&mut reader, 32).await.unwrap().unwrap();
    assert_eq!(parse_request(&line).unwrap(), Request::Table);
}
