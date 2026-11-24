//! The link mode (S5), on with `ANIMA_LINK_SOCKET`: the hub listens on loopback and that socket
//! only (NET-003), and permits this computer and the devices of the link (NET-001).

use std::{
    collections::BTreeMap,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr},
    path::{Path, PathBuf},
};

use ipnet::{IpNet, Ipv4Net, Ipv6Net};

use super::{Error, PERMITTED_NETWORKS, optional_socket};

pub(super) const LINK_SOCKET: &str = "ANIMA_LINK_SOCKET";
pub(super) const LINK_CONTROL: &str = "ANIMA_LINK_CONTROL";
pub(super) const HOSTED_CLOUD_KEY_FILE: &str = "ANIMA_HOSTED_CLOUD_KEY_FILE";

/// The names `anima-link` gives permitted device keys; no packet goes to one.
/// `link/devices/address.go` holds the same range.
pub const LINK_DEVICES: Ipv6Net =
    Ipv6Net::new_assert(Ipv6Addr::new(0xfd41, 0x4e49, 0x4d41, 0, 0, 0, 0, 0), 48);

/// In the link mode a `listen` address off loopback is refused, and so is a list of permitted
/// networks: the link permits devices by key.
pub(super) fn socket(
    env: &BTreeMap<String, String>,
    listen: SocketAddr,
) -> Result<Option<PathBuf>, Error> {
    let Some(socket) = optional_socket(env, LINK_SOCKET)? else {
        return Ok(None);
    };
    if !listen.ip().to_canonical().is_loopback() {
        return Err(Error::LinkListen { listen });
    }
    if env
        .get(PERMITTED_NETWORKS)
        .is_some_and(|set| !set.is_empty())
    {
        return Err(Error::LinkNetworks);
    }
    Ok(Some(socket))
}

/// Refused outside the link mode: a hub that no link reaches has no link devices (FRN-009).
pub(super) fn control(
    env: &BTreeMap<String, String>,
    link_mode: bool,
) -> Result<Option<PathBuf>, Error> {
    let control = optional_socket(env, LINK_CONTROL)?;
    if control.is_some() && !link_mode {
        return Err(Error::LinkControlAlone);
    }
    Ok(control)
}

/// Beside the control socket, where the link makes both (HMD-048, `link/cmd/anima-link/hub.go`).
#[must_use]
pub fn dial_socket(control: &Path) -> Option<PathBuf> {
    Some(control.parent()?.join("dial.sock"))
}

/// Refused without the control socket, through which the hub makes the cloud's invites (R-129).
pub(super) fn hosted(
    env: &BTreeMap<String, String>,
    control_set: bool,
) -> Result<Option<PathBuf>, Error> {
    let file = optional_socket(env, HOSTED_CLOUD_KEY_FILE)?;
    if file.is_some() && !control_set {
        return Err(Error::HostedWithoutLink);
    }
    Ok(file)
}

pub(super) fn permitted_networks() -> Vec<IpNet> {
    vec![
        IpNet::V4(Ipv4Net::new_assert(Ipv4Addr::new(127, 0, 0, 0), 8)),
        IpNet::V6(Ipv6Net::new_assert(Ipv6Addr::LOCALHOST, 128)),
        IpNet::V6(LINK_DEVICES),
    ]
}

pub(super) const LISTEN_RULE: &str = "with ANIMA_LINK_SOCKET the hub listens on loopback and \
                                      the link only (NET-003)";
pub(super) const NETWORKS_RULE: &str = "must not be set with ANIMA_LINK_SOCKET: the link \
                                        permits devices by key (NET-001)";
pub(super) const HOSTED_RULE: &str = "must be set only with ANIMA_LINK_CONTROL: a hosted hub \
                                       makes its device invites through the link (R-129)";
pub(super) const CONTROL_RULE: &str = "must be set only with ANIMA_LINK_SOCKET: the control \
                                       socket belongs to the link mode (FRN-009)";

#[cfg(test)]
mod tests {
    use super::super::{
        Error, LISTEN, from_env,
        tests::{env, host_path},
    };
    use super::*;

    #[cfg(unix)]
    const SOCKET: &str = "/run/anima/link.sock";
    #[cfg(windows)]
    const SOCKET: &str = "C:/run/anima/link.sock";

    #[test]
    fn without_the_socket_the_hub_is_not_in_the_link_mode_and_nothing_changes() {
        for unset in [env(&[]), env(&[(LINK_SOCKET, "")])] {
            let config = from_env(&unset).unwrap();
            assert_eq!(config.link_socket, None);
            assert_eq!(
                config.permitted_networks,
                vec!["100.64.0.0/10".parse().unwrap()]
            );
        }
        let vpn = env(&[
            (LISTEN, "100.64.0.1:443"),
            (PERMITTED_NETWORKS, "10.0.0.0/8"),
        ]);
        let config = from_env(&vpn).unwrap();
        assert_eq!(config.listen, "100.64.0.1:443".parse().unwrap());
        assert_eq!(
            config.permitted_networks,
            vec!["10.0.0.0/8".parse().unwrap()]
        );
    }

    #[test]
    fn net_001_in_the_link_mode_the_clients_are_this_computer_and_the_links_devices() {
        let config = from_env(&env(&[(LINK_SOCKET, SOCKET)])).unwrap();
        assert_eq!(config.link_socket, Some(PathBuf::from(SOCKET)));
        let permitted: Vec<String> = config
            .permitted_networks
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(permitted, ["127.0.0.0/8", "::1/128", "fd41:4e49:4d41::/48"]);
    }

    #[test]
    fn net_003_in_the_link_mode_the_hub_listens_on_loopback_only() {
        for loopback in ["127.0.0.1:8443", "[::1]:8443", "[::ffff:127.0.0.1]:8443"] {
            let config = from_env(&env(&[(LINK_SOCKET, SOCKET), (LISTEN, loopback)]));
            assert!(config.is_ok(), "{loopback}");
        }
        for other in [
            "100.64.0.1:443",
            "10.0.0.5:8443",
            "192.168.1.9:8443",
            "[fd00::1]:8443",
        ] {
            let error = from_env(&env(&[(LINK_SOCKET, SOCKET), (LISTEN, other)])).unwrap_err();
            assert!(matches!(error, Error::LinkListen { .. }), "{other}");
            assert!(error.to_string().contains("NET-003"), "{other}");
        }
    }

    #[test]
    fn net_001_in_the_link_mode_a_list_of_permitted_networks_is_refused() {
        let both = env(&[(LINK_SOCKET, SOCKET), (PERMITTED_NETWORKS, "0.0.0.0/0")]);
        assert!(matches!(from_env(&both).unwrap_err(), Error::LinkNetworks));
        // Compose passes an unset variable as empty.
        let empty = env(&[(LINK_SOCKET, SOCKET), (PERMITTED_NETWORKS, "")]);
        assert!(from_env(&empty).is_ok());
    }

    #[test]
    fn frn_009_the_control_socket_is_read_in_the_link_mode_only() {
        let control = &host_path("/run/anima/link/control.sock");
        let both = from_env(&env(&[(LINK_SOCKET, SOCKET), (LINK_CONTROL, control)])).unwrap();
        assert_eq!(both.link_control, Some(PathBuf::from(control)));
        let socket_only = from_env(&env(&[(LINK_SOCKET, SOCKET), (LINK_CONTROL, "")])).unwrap();
        assert_eq!(socket_only.link_control, None);
        let alone = from_env(&env(&[(LINK_CONTROL, control)])).unwrap_err();
        assert!(matches!(alone, Error::LinkControlAlone), "{alone}");
        assert!(alone.to_string().contains("ANIMA_LINK_SOCKET"));
        let relative = from_env(&env(&[
            (LINK_SOCKET, SOCKET),
            (LINK_CONTROL, "control.sock"),
        ]));
        assert!(matches!(
            relative.unwrap_err(),
            Error::NotAbsolute {
                variable: LINK_CONTROL,
                ..
            }
        ));
    }

    #[test]
    fn hmd_048_the_dial_socket_is_beside_the_control_socket() {
        assert_eq!(
            dial_socket(Path::new("/run/anima/link/control.sock")),
            Some(PathBuf::from("/run/anima/link/dial.sock"))
        );
    }

    #[test]
    fn the_socket_must_be_an_absolute_path() {
        let error = from_env(&env(&[(LINK_SOCKET, "link.sock")])).unwrap_err();
        assert!(matches!(
            error,
            Error::NotAbsolute {
                variable: LINK_SOCKET,
                ..
            }
        ));
    }

    #[test]
    fn r_129_the_hosted_mode_needs_the_control_socket_of_the_link() {
        let key = &host_path("/run/secrets/anima/cloud_key");
        let hosted = env(&[
            (LINK_SOCKET, SOCKET),
            (LINK_CONTROL, &host_path("/run/anima/link-control.sock")),
            (HOSTED_CLOUD_KEY_FILE, key),
            ("ANIMA_PROBE_DENY", "10.130.6.0/24"),
        ]);
        assert_eq!(
            from_env(&hosted).unwrap().hosted_cloud_key_file,
            Some(PathBuf::from(key))
        );
        let alone = env(&[(LINK_SOCKET, SOCKET), (HOSTED_CLOUD_KEY_FILE, key)]);
        assert!(matches!(from_env(&alone), Err(Error::HostedWithoutLink)));
        assert_eq!(from_env(&env(&[])).unwrap().hosted_cloud_key_file, None);
    }

    #[test]
    fn r_030_a_hosted_hub_does_not_start_with_no_networks_to_refuse() {
        let key = &host_path("/run/secrets/anima/cloud_key");
        let control = &host_path("/run/anima/link-control.sock");
        for denied in [None, Some("")] {
            let mut pairs = vec![
                (LINK_SOCKET, SOCKET),
                (LINK_CONTROL, control.as_str()),
                (HOSTED_CLOUD_KEY_FILE, key.as_str()),
            ];
            pairs.extend(denied.map(|text| ("ANIMA_PROBE_DENY", text)));
            assert!(
                matches!(from_env(&env(&pairs)), Err(Error::HostedWithoutProbeDeny)),
                "{denied:?}"
            );
        }
        let local = env(&[(LINK_SOCKET, SOCKET)]);
        assert_eq!(
            from_env(&local).unwrap().probe_deny,
            vec![],
            "a hub that is not hosted"
        );
    }
}
