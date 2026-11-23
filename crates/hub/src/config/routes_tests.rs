//! The settings of the routes (HOST-050, task H40; SF-327).

use super::tests::env;
use super::*;

#[test]
fn the_ports_of_streams_are_off_by_default_and_never_below_1024() {
    assert_eq!(from_env(&env(&[])).unwrap().routes.stream_ports, None);
    let set = from_env(&env(&[("ANIMA_ROUTES_STREAM_PORTS", "20000-20999")])).unwrap();
    assert_eq!(set.routes.stream_ports, Some(20_000..=20_999));
    for refused in ["993-2000", "21000-20000", "20000", "a-b"] {
        assert!(
            from_env(&env(&[("ANIMA_ROUTES_STREAM_PORTS", refused)])).is_err(),
            "{refused}"
        );
    }
}

#[test]
fn the_streams_of_one_device_stay_below_the_streams_of_its_link() {
    let routes = from_env(&env(&[])).unwrap().routes;
    assert_eq!(
        (routes.streams_per_device, routes.streams_per_link),
        (16, 32)
    );
    for (device, link) in [("32", "32"), ("40", "32"), ("0", "32")] {
        let pairs = [
            ("ANIMA_ROUTES_STREAMS_PER_DEVICE", device),
            ("ANIMA_ROUTES_STREAMS_PER_LINK", link),
        ];
        assert!(from_env(&env(&pairs)).is_err(), "{device} of {link}");
    }
}
