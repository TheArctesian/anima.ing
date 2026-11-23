//! The hub's own daemon in the hosted mode (HMD-071): K8 routes on a private Unix socket, and one
//! enrolment code in a private file.

use std::{collections::BTreeMap, path::PathBuf};

use super::{Error, optional_socket};

pub(super) const OWN_DAEMON_SOCKET: &str = "ANIMA_OWN_DAEMON_SOCKET";
pub(super) const OWN_DAEMON_CODE_FILE: &str = "ANIMA_OWN_DAEMON_CODE_FILE";

pub(super) const OWN_DAEMON_RULE: &str = "must be set only with ANIMA_HOSTED_CLOUD_KEY_FILE: \
                                           only a hosted hub has a daemon of its own (HMD-071)";
pub(super) const CODE_FILE_RULE: &str = "must be set only with ANIMA_OWN_DAEMON_SOCKET: the \
                                          code is for the daemon that reaches the hub there";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnDaemon {
    /// The socket that serves only the routes of K8, each request as from the hub's device.
    pub socket: PathBuf,
    /// One enrolment code of the hub's device, written at the start; `None` when the daemon has
    /// its token.
    pub code_file: Option<PathBuf>,
}

pub(super) fn own_daemon(
    env: &BTreeMap<String, String>,
    hosted: bool,
) -> Result<Option<OwnDaemon>, Error> {
    let socket = optional_socket(env, OWN_DAEMON_SOCKET)?;
    let code_file = optional_socket(env, OWN_DAEMON_CODE_FILE)?;
    match (socket, code_file) {
        (None, None) => Ok(None),
        (None, Some(_)) => Err(Error::OwnDaemonCodeAlone),
        (Some(_), _) if !hosted => Err(Error::OwnDaemonNotHosted),
        (Some(socket), code_file) => Ok(Some(OwnDaemon { socket, code_file })),
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        Error, from_env,
        tests::{env, host_path},
    };
    use super::*;

    fn hosted(more: &[(&'static str, &str)]) -> BTreeMap<String, String> {
        let socket = host_path("/run/anima/link.sock");
        let control = host_path("/run/anima/link-control.sock");
        let key = host_path("/etc/anima/cloud.key");
        let mut all = vec![
            ("ANIMA_LINK_SOCKET", socket.as_str()),
            ("ANIMA_LINK_CONTROL", control.as_str()),
            ("ANIMA_HOSTED_CLOUD_KEY_FILE", key.as_str()),
        ];
        all.extend_from_slice(more);
        env(&all)
    }

    #[test]
    fn hmd_071_a_hosted_hub_takes_the_socket_and_the_code_file_of_its_own_daemon() {
        let socket = host_path("/run/anima/own-daemon.sock");
        let code = host_path("/run/anima/own-daemon.code");
        let both = hosted(&[(OWN_DAEMON_SOCKET, &socket), (OWN_DAEMON_CODE_FILE, &code)]);
        let expected = OwnDaemon {
            socket: PathBuf::from(&socket),
            code_file: Some(PathBuf::from(&code)),
        };
        assert_eq!(from_env(&both).unwrap().own_daemon, Some(expected));
        let enrolled = from_env(&hosted(&[(OWN_DAEMON_SOCKET, &socket)])).unwrap();
        assert_eq!(enrolled.own_daemon.unwrap().code_file, None);
        assert_eq!(from_env(&hosted(&[])).unwrap().own_daemon, None);
    }

    #[test]
    fn hmd_071_only_a_hosted_hub_has_an_own_daemon_and_a_code_needs_the_socket() {
        let socket = host_path("/run/anima/own-daemon.sock");
        let code = host_path("/run/anima/own-daemon.code");
        let local = env(&[(OWN_DAEMON_SOCKET, &socket)]);
        assert!(matches!(from_env(&local), Err(Error::OwnDaemonNotHosted)));
        let alone = hosted(&[(OWN_DAEMON_CODE_FILE, &code)]);
        assert!(matches!(from_env(&alone), Err(Error::OwnDaemonCodeAlone)));
    }
}
