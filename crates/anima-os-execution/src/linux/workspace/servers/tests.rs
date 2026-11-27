use super::*;
use std::fmt::Write as _;

#[test]
fn discovery_keeps_only_unique_listeners_that_the_ipv4_connector_can_reach() {
    let network = |address: [u8; 4]| format!("{:08X}", u32::from_ne_bytes(address));
    let localhost = network([127, 0, 0, 1]);
    let ipv4_mapped = format!("0000000000000000{}{localhost}", network([0, 0, 255, 255]));
    let tcp = [
        (localhost.as_str(), "0BB8", "0A"), // loopback listener on 3000
        (localhost.as_str(), "0BB8", "0A"), // duplicate
        ("00000000", "1F90", "0A"),         // wildcard listener on 8080
        (&network([10, 1, 0, 2]), "0BB9", "0A"),
        (localhost.as_str(), "0BBA", "01"), // established, not listening
        ("00000000000000000000000000000000", "0BBB", "0A"), // IPv6 wildcard
        (ipv4_mapped.as_str(), "1111", "0A"),
    ];
    let mut tables = String::new();
    for (addr, port, state) in tcp {
        writeln!(
            tables,
            " 0: {addr}:{port} 00000000:0000 {state} 00000000:00000000"
        )
        .unwrap();
    }
    assert_eq!(listening(&tables), [3000, 4369, 8080]);
}
