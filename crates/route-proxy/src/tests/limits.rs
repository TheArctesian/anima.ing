//! The limits of a name (Section 12.7).

use std::net::IpAddr;

use crate::limits::{Bucket, Caps, Gate, Limiter, Limits, Refusal, client_key};

const LIMITS: Limits = Limits {
    connections: 2,
    new_per_second: 3,
    bytes_per_period: 1000,
    period_ms: 60_000,
};

#[test]
fn connections_at_one_time_are_limited_and_a_release_frees_one() {
    let mut limiter = Limiter::new(LIMITS);
    limiter.admit("a.example.com", 0).unwrap();
    limiter.admit("a.example.com", 0).unwrap();
    assert_eq!(limiter.admit("a.example.com", 0), Err(Refusal::Connections));
    assert_eq!(
        limiter.admit("b.example.com", 0),
        Ok(()),
        "each name has its own limits"
    );
    limiter.release("a.example.com");
    assert_eq!(limiter.admit("a.example.com", 0), Ok(()));
}

#[test]
fn new_connections_each_second_are_limited_and_refill_with_time() {
    let mut limiter = Limiter::new(Limits {
        connections: 100,
        ..LIMITS
    });
    for _ in 0..3 {
        limiter.admit("a.example.com", 0).unwrap();
    }
    assert_eq!(limiter.admit("a.example.com", 100), Err(Refusal::Rate));
    assert_eq!(limiter.admit("a.example.com", 400), Ok(()));
}

#[test]
fn the_bytes_of_a_period_are_limited_until_the_next_period() {
    let mut limiter = Limiter::new(Limits {
        connections: 100,
        new_per_second: 100,
        ..LIMITS
    });
    limiter.admit("a.example.com", 0).unwrap();
    assert!(limiter.add_bytes("a.example.com", 600, 10));
    assert!(!limiter.add_bytes("a.example.com", 600, 20));
    assert_eq!(limiter.admit("a.example.com", 30), Err(Refusal::Bytes));
    assert_eq!(limiter.admit("a.example.com", 60_030), Ok(()));
}

#[test]
fn a_listener_caps_its_connections_in_all_and_for_each_client() {
    let mut gate = Gate::new(Caps {
        total: 3,
        per_client: 2,
    });
    let one: IpAddr = "198.51.100.1".parse().unwrap();
    let two: IpAddr = "198.51.100.2".parse().unwrap();
    assert!(gate.enter() && gate.enter_client(one));
    assert!(gate.enter() && gate.enter_client(one));
    assert!(gate.enter());
    assert!(!gate.enter_client(one), "a client has its own cap");
    assert!(gate.enter_client(two));
    assert!(!gate.enter(), "the cap in all");
    gate.leave();
    gate.leave_client(one);
    assert!(gate.enter() && gate.enter_client(one));
}

#[test]
fn a_client_is_its_64_of_ipv6_and_its_address_of_ipv4() {
    let one: IpAddr = "2001:db8:1:2:aaaa::1".parse().unwrap();
    let two: IpAddr = "2001:db8:1:2:bbbb::9".parse().unwrap();
    let other: IpAddr = "2001:db8:1:3::1".parse().unwrap();
    assert_eq!(client_key(one), client_key(two));
    assert_ne!(client_key(one), client_key(other));
    let v4: IpAddr = "198.51.100.7".parse().unwrap();
    assert_eq!(client_key(v4), v4);
    let mapped: IpAddr = "::ffff:198.51.100.7".parse().unwrap();
    assert_eq!(
        client_key(mapped),
        v4,
        "an IPv4 address in IPv6 form is IPv4"
    );
}

#[test]
fn the_addresses_of_one_64_share_the_cap_of_one_client() {
    let mut gate = Gate::new(Caps {
        total: 100,
        per_client: 2,
    });
    let addresses: Vec<IpAddr> = ["2001:db8::1", "2001:db8::2", "2001:db8::3"]
        .iter()
        .map(|text| text.parse().unwrap())
        .collect();
    assert!(gate.enter_client(addresses[0]));
    assert!(gate.enter_client(addresses[1]));
    assert!(
        !gate.enter_client(addresses[2]),
        "a new address in the same /64 is the same client"
    );
    gate.leave_client(addresses[0]);
    assert!(gate.enter_client(addresses[2]));
}

#[test]
fn a_bucket_takes_its_burst_at_once_and_then_its_rate() {
    let mut arrivals = Bucket::full(8, 16, 0);
    assert_eq!(
        (0..20).filter(|_| arrivals.take(0)).count(),
        16,
        "the burst"
    );
    assert!(!arrivals.take(100), "0.8 of an arrival after 100 ms");
    assert!(arrivals.take(125), "one after 125 ms");
    assert_eq!(
        (0..20).filter(|_| arrivals.take(1125)).count(),
        8,
        "8 in the next second"
    );
    assert_eq!(
        (0..40).filter(|_| arrivals.take(60_000)).count(),
        16,
        "never more than the burst"
    );
}
