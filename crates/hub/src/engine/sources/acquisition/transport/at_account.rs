//! The request of a source whose server is of the operator (K-E1 Section 7b, R-261): the host of
//! the base is resolved one time and checked with the address policy of the hub before the key
//! is sent.

use std::net::{IpAddr, SocketAddr};

use reqwest::{
    Client, Url,
    header::{ACCEPT, AUTHORIZATION, HeaderValue},
    redirect::Policy,
};
use source_exchange::{Method, Request};

use super::{Answer, CONNECT_TIMEOUT, Fault, TIMEOUT, Transport, address, bearer, read};
use crate::fleet::{
    credentials::Secret,
    targets::{TargetError, resolve_checked},
};

impl Transport {
    /// A transport made for a loopback stub of the tests sends plain `http` to any address.
    const fn sends_plain_http_anywhere(&self) -> bool {
        self.base_override.is_some()
    }

    /// The connection goes only to an address that the policy permitted; the name is not
    /// resolved a second time, and no redirect is followed.
    ///
    /// # Errors
    /// [`Fault::Refused`] for a path that could leave the base, an address that the policy
    /// refuses, or a plain `http` base with an address that is not private or VPN (OD-E26):
    /// nothing is sent. [`Fault::Network`] when the name
    /// does not resolve or the server does not answer.
    pub(in crate::engine::sources) async fn get_at(
        &self,
        base: &Url,
        request: &Request,
        key: &Secret,
    ) -> Result<Answer, Fault> {
        let Method::Get = request.method;
        let plain = base.scheme() == "http";
        if !plain && base.scheme() != "https" {
            return Err(Fault::Refused);
        }
        let url = address(base, &request.path, &request.query)?;
        let host = base.host_str().ok_or(Fault::Refused)?;
        let port = base.port_or_known_default().ok_or(Fault::Refused)?;
        let addresses =
            resolve_checked(host, port, &self.policy)
                .await
                .map_err(|error| match error {
                    TargetError::Refused => Fault::Refused,
                    TargetError::Unresolved => Fault::Network,
                })?;
        if plain && !self.sends_plain_http_anywhere() && !all_private(&addresses) {
            return Err(Fault::Refused);
        }
        let mut client = Client::builder()
            .use_rustls_tls()
            .user_agent("anima-hub")
            .no_proxy()
            .redirect(Policy::none())
            .https_only(!plain)
            .timeout(TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT);
        let literal = host.starts_with('[') || host.parse::<IpAddr>().is_ok();
        if !literal {
            client = client.resolve_to_addrs(host, &addresses);
        }
        let client = client.build().map_err(|_| Fault::Network)?;
        let sent = client
            .get(url)
            .header(AUTHORIZATION, bearer(key.expose())?)
            .header(ACCEPT, HeaderValue::from_static("application/json"));
        read(sent.send().await.map_err(|_| Fault::Network)?).await
    }
}

/// Each address is in a private or VPN range (OD-E26): 10/8, 172.16/12, 192.168/16,
/// 100.64/10 or `fc00::/7`. An empty list is not.
#[must_use]
pub fn all_private(addresses: &[SocketAddr]) -> bool {
    !addresses.is_empty()
        && addresses
            .iter()
            .all(|address| match address.ip().to_canonical() {
                IpAddr::V4(ip) => {
                    let [a, b, ..] = ip.octets();
                    ip.is_private() || (a == 100 && (64..128).contains(&b))
                }
                IpAddr::V6(ip) => (ip.segments()[0] & 0xfe00) == 0xfc00,
            })
}

#[cfg(test)]
mod tests {
    use super::all_private;

    fn at(texts: &[&str]) -> Vec<std::net::SocketAddr> {
        texts.iter().map(|t| t.parse().unwrap()).collect()
    }

    #[test]
    fn od_e26_plain_http_goes_only_to_private_or_vpn_addresses() {
        for private in [
            "10.0.0.5:80",
            "172.16.0.1:80",
            "172.31.255.255:80",
            "192.168.1.20:3000",
            "100.64.0.1:80",
            "100.127.255.255:80",
            "[fd7a:115c:a1e0::1]:80",
            "[fc00::1]:80",
            "[::ffff:192.168.1.2]:80",
        ] {
            assert!(all_private(&at(&[private])), "{private}");
        }
        for public in [
            "8.8.8.8:80",
            "172.32.0.1:80",
            "100.128.0.1:80",
            "100.63.255.255:80",
            "192.169.0.1:80",
            "[2001:db8::1]:80",
            "[fe80::1]:80",
            "127.0.0.1:80",
            "[::ffff:8.8.8.8]:80",
        ] {
            assert!(!all_private(&at(&[public])), "{public}");
        }
        assert!(
            !all_private(&at(&["10.0.0.5:80", "8.8.8.8:80"])),
            "one public address"
        );
        assert!(!all_private(&[]));
    }
}
