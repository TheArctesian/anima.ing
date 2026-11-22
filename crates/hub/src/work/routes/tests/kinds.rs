//! The kinds and the access of a route (HOST-050, task H40; SF-248, SF-245):
//! what each kind needs, the port of the hub of a stream, the protocols of a public stream, the
//! hosted hub, and the address of a redirect.

use super::support::{stream, stream_rules, web};
use crate::work::routes::kinds::{
    Access, Kind, KindRefusal, Shape, StreamRules, check, redirect_to,
};

fn redirect(to: &str) -> Shape {
    Shape {
        kind: Kind::Redirect,
        access: Access::Public,
        port: None,
        listen_port: None,
        protocol: None,
        redirect_to: Some(to.to_owned()),
    }
}

fn with_protocol(mut shape: Shape, protocol: &str) -> Shape {
    shape.protocol = Some(protocol.to_owned());
    shape
}

#[test]
fn a_web_route_has_a_port_and_nothing_of_a_stream() {
    assert!(check(&web(3000), &stream_rules()).is_ok());
    let mut listening = web(3000);
    listening.listen_port = Some(20_001);
    assert_eq!(
        check(&listening, &stream_rules()),
        Err(KindRefusal::ListenPortNotForKind)
    );
    let mut none = web(3000);
    none.port = None;
    assert_eq!(check(&none, &stream_rules()), Err(KindRefusal::NoPort));
}

#[test]
fn a_private_stream_needs_a_port_of_the_hub_in_the_range_and_no_protocol() {
    let shape = stream(Kind::TcpStream, Access::Private, 20_022, 6000);
    let checked = check(&shape, &stream_rules()).unwrap();
    assert_eq!(
        (checked.listen_port, checked.protocol),
        (Some(20_022), None)
    );
    let mut bare = shape.clone();
    bare.listen_port = None;
    assert_eq!(
        check(&bare, &stream_rules()),
        Err(KindRefusal::NoListenPort)
    );
}

#[test]
fn the_ports_of_the_hub_below_1024_reserved_or_outside_the_range_are_refused() {
    let wide = StreamRules {
        ports: Some(1..=65_535),
        ..stream_rules()
    };
    for (port, refusal) in [
        (993, KindRefusal::ListenPortBelow1024),
        (22, KindRefusal::ListenPortBelow1024),
        (5432, KindRefusal::ListenPortReserved),
        (41_641, KindRefusal::ListenPortReserved),
        (51_820, KindRefusal::ListenPortReserved),
        (8443, KindRefusal::ListenPortReserved),
        (4822, KindRefusal::ListenPortReserved),
        (3478, KindRefusal::ListenPortReserved),
    ] {
        let shape = stream(Kind::TcpStream, Access::Private, port, 6000);
        assert_eq!(check(&shape, &wide), Err(refusal), "{port}");
    }
    let outside = stream(Kind::TcpStream, Access::Private, 30_000, 6000);
    assert_eq!(
        check(&outside, &stream_rules()),
        Err(KindRefusal::ListenPortOutsideRange)
    );
    let none = StreamRules {
        ports: None,
        ..stream_rules()
    };
    let inside = stream(Kind::TcpStream, Access::Private, 20_022, 6000);
    assert_eq!(
        check(&inside, &none),
        Err(KindRefusal::NoStreamPorts),
        "off by default"
    );
}

#[test]
fn a_hosted_hub_has_no_stream_until_the_door_has_an_address_for_it() {
    let hosted = StreamRules {
        hosted: true,
        ..stream_rules()
    };
    let shape = stream(Kind::TcpStream, Access::Private, 20_022, 6000);
    assert_eq!(check(&shape, &hosted), Err(KindRefusal::StreamOnHostedHub));
    assert!(check(&web(3000), &hosted).is_ok(), "a web route stays");
}

#[test]
fn a_public_stream_needs_a_protocol_with_its_own_sign_in_and_tls() {
    let public = stream(Kind::TcpStream, Access::Public, 20_993, 993);
    assert!(matches!(
        check(&public, &stream_rules()),
        Err(KindRefusal::PublicStreamProtocol(list)) if list.contains("imaps (usual port 993)")
    ));
    let imaps = check(&with_protocol(public.clone(), "IMAPS"), &stream_rules()).unwrap();
    assert_eq!(imaps.protocol, Some("imaps"));
    for refused in ["redis", "mqtt", "postgres", "ssh", "syslog", "wireguard"] {
        let shape = with_protocol(public.clone(), refused);
        assert!(
            check(&shape, &stream_rules()).is_err(),
            "{refused} over TCP"
        );
    }
    let udp = stream(Kind::UdpStream, Access::Public, 20_820, 51_821);
    assert!(check(&with_protocol(udp.clone(), "wireguard"), &stream_rules()).is_ok());
    assert!(
        check(&with_protocol(udp, "syslog"), &stream_rules()).is_err(),
        "no public syslog"
    );
}

#[test]
fn a_redirect_has_one_https_address_and_no_port() {
    let checked = check(&redirect("https://Photos.Example.org/new"), &stream_rules()).unwrap();
    assert_eq!(
        checked.redirect_to.as_deref(),
        Some("https://photos.example.org/new")
    );
    let mut with_port = redirect("https://photos.example.org/");
    with_port.port = Some(3000);
    assert_eq!(
        check(&with_port, &stream_rules()),
        Err(KindRefusal::RedirectShape)
    );
    let mut web_to = web(3000);
    web_to.redirect_to = Some("https://a.example".to_owned());
    assert_eq!(
        check(&web_to, &stream_rules()),
        Err(KindRefusal::RedirectToNotForKind)
    );
}

#[test]
fn the_address_of_a_redirect_is_a_plain_https_address() {
    assert!(redirect_to("https://example.org:8443/a?b=c").is_ok());
    for refused in [
        "http://example.org/",
        "https://user@example.org/",
        "https://exa mple.org/",
        "https://example.org/\r\nset-cookie: a=b",
        "https://-bad.example/",
        "https://example.org:0/",
        "https:///path",
        "https://example.org\\@evil.example/",
        "javascript:alert(1)",
    ] {
        assert_eq!(
            redirect_to(refused),
            Err(KindRefusal::RedirectTo),
            "{refused}"
        );
    }
}

#[test]
fn a_hub_that_is_not_on_unix_refuses_routing_in_plain_words() {
    assert!(crate::work::routes::routing_here(true).is_ok());
    let refused = crate::work::routes::routing_here(false).unwrap_err();
    assert!(matches!(refused, crate::work::routes::Error::NeedsLinuxHub));
    assert_eq!(refused.to_string(), "Routing needs a hub on Linux.");
}
