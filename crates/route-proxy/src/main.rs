//! The reverse proxy of the public names of applications, a process apart from the hub (task H21).
//! Its interface to the hub is a Unix socket, so it runs only on Unix (R-366).

#[cfg(unix)]
mod running {

    use std::{net::SocketAddr, process::ExitCode, sync::Arc, time::Duration};

    use anima_route_proxy::{
        hub::HubSocket,
        serve::{Proxy, http, https},
        settings,
        wire::Reach,
    };
    use tokio::net::TcpListener;

    const REFRESH: Duration = Duration::from_secs(60);

    #[tokio::main]
    pub async fn main() -> ExitCode {
        tracing_subscriber::fmt().json().init();
        let env = std::env::vars()
            .filter(|(name, _)| name.starts_with("ANIMA_"))
            .collect();
        let settings = match settings::from_env(&env) {
            Ok(Some(settings)) => settings,
            Ok(None) => {
                eprintln!("anima-route-proxy is off: ANIMA_ROUTE_PROXY_LISTEN is not set");
                return ExitCode::SUCCESS;
            }
            Err(error) => {
                eprintln!("anima-route-proxy: {error}");
                return ExitCode::from(2);
            }
        };
        let Some(listener) = bind(settings.listen).await else {
            return ExitCode::FAILURE;
        };
        let mut plain = None;
        if let Some(address) = settings.listen_plain {
            let Some(listener) = bind(address).await else {
                return ExitCode::FAILURE;
            };
            plain = Some(listener);
        }
        let mut private = None;
        if let Some(place) = &settings.private {
            let Some(listener) = bind(SocketAddr::new(place.address, settings.listen.port())).await
            else {
                return ExitCode::FAILURE;
            };
            private = Some(listener);
        }
        let hub = HubSocket(settings.hub_socket.clone());
        let Ok(proxy) = Proxy::new(hub, &settings) else {
            return ExitCode::FAILURE;
        };
        let proxy = Arc::new(proxy);
        if let Some(plain) = plain {
            tokio::spawn(http::serve(Arc::clone(&proxy), plain));
        }
        if let Some(private) = private {
            tokio::spawn(https::serve(Arc::clone(&proxy), private, Reach::Private));
        }
        tokio::spawn(https::serve(Arc::clone(&proxy), listener, Reach::Public));
        loop {
            if let Err(error) = proxy.refresh().await {
                tracing::warn!(error = %error, "the table of the hub was not read");
            }
            proxy.keep_streams().await;
            proxy.report().await;
            tokio::time::sleep(REFRESH).await;
        }
    }

    async fn bind(address: SocketAddr) -> Option<TcpListener> {
        let bound = TcpListener::bind(address).await.ok();
        if bound.is_none() {
            eprintln!(
                "anima-route-proxy: {address} is in use by a different program, or not permitted"
            );
        }
        bound
    }
}

#[cfg(unix)]
fn main() -> std::process::ExitCode {
    running::main()
}

#[cfg(not(unix))]
fn main() -> std::process::ExitCode {
    eprintln!("anima-route-proxy: Routing needs a hub on Linux.");
    std::process::ExitCode::FAILURE
}
