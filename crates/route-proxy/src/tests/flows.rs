//! The flows of a UDP stream (task H41, SF-327): false sources of many networks replace only
//! each other's waiting flows, and a flow whose source answered keeps its place.

use std::net::SocketAddr;

use crate::streams::flows::{FlowCaps, FlowTable};

const CAPS: FlowCaps = FlowCaps {
    total: 8,
    per_network: 4,
    waiting: 4,
};

/// A source in its own /24, so the cap of one network does not apply.
fn spread(n: u32) -> SocketAddr {
    let [a, b, c, _] = (0x0a00_0000 + (n << 8)).to_be_bytes();
    SocketAddr::from(([a, b, c, 7], 40_000))
}

#[test]
fn false_sources_of_many_networks_replace_only_waiting_flows() {
    let mut table = FlowTable::new(CAPS);
    let real = spread(0);
    let opened = table.open(real, "real").unwrap();
    assert!(
        table.set_up(real, opened.id),
        "the source answered after the application"
    );
    let mut replaced = Vec::new();
    for n in 1..=1_000 {
        let opened = table.open(spread(n), "false").unwrap();
        replaced.extend(opened.replaced);
    }
    assert_eq!(
        table.get(real),
        Some(&"real"),
        "the set-up flow keeps its place"
    );
    assert_eq!(
        table.counts(),
        (5, 1),
        "one set-up flow and the cap of waiting flows"
    );
    assert_eq!(
        replaced.len(),
        1_000 - 4,
        "each false source after the cap replaced one"
    );
    assert!(replaced.iter().all(|handle| *handle == "false"));
}

#[test]
fn a_new_real_source_gets_a_waiting_place_while_false_sources_arrive() {
    let mut table = FlowTable::new(CAPS);
    for n in 1..=4 {
        table.open(spread(n), "false").unwrap();
    }
    let real = spread(500);
    let opened = table.open(real, "real").unwrap();
    assert_eq!(
        opened.replaced,
        Some("false"),
        "the oldest waiting flow goes"
    );
    for n in 5..=7 {
        table.open(spread(n), "false").unwrap();
    }
    assert_eq!(
        table.get(real),
        Some(&"real"),
        "three later arrivals did not reach it"
    );
    assert!(table.set_up(real, opened.id));
    for n in 8..=100 {
        table.open(spread(n), "false").unwrap();
    }
    assert_eq!(table.get(real), Some(&"real"));
}

#[test]
fn a_table_of_set_up_flows_refuses_a_new_source() {
    let mut table = FlowTable::new(CAPS);
    for n in 0..8 {
        let opened = table.open(spread(n), n).unwrap();
        table.set_up(spread(n), opened.id);
    }
    assert!(
        table.open(spread(9), 9).is_none(),
        "no waiting flow to replace"
    );
    assert_eq!(table.counts(), (8, 8));
}

#[test]
fn one_source_network_keeps_its_cap() {
    let mut table = FlowTable::new(CAPS);
    let host = |last: u8| SocketAddr::from(([198, 51, 100, last], 40_000));
    for last in 1..=4 {
        assert!(table.open(host(last), last).is_some());
    }
    assert!(table.open(host(5), 5).is_none(), "a fifth flow of one /24");
    assert!(
        table.open(host(1), 1).is_none(),
        "a second flow of one source"
    );
}

#[test]
fn closing_an_old_flow_leaves_a_later_flow_of_the_same_source() {
    let mut table = FlowTable::new(CAPS);
    let source = spread(1);
    let first = table.open(source, "first").unwrap();
    table.close(source, first.id);
    let second = table.open(source, "second").unwrap();
    table.close(source, first.id);
    assert_eq!(table.get(source), Some(&"second"));
    assert!(
        !table.set_up(source, first.id),
        "the old number does not set up the new flow"
    );
    assert!(table.set_up(source, second.id));
}
