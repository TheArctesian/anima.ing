//! A desktop loopback hub origin is `http://127.0.0.1:<port>` only; `localhost` and `[::1]` are refused (F-1).

pub fn port(origin: &str) -> Option<u16> {
    let port = origin.strip_prefix("http://127.0.0.1:")?;
    let digits =
        !port.is_empty() && port.len() <= 5 && port.bytes().all(|byte| byte.is_ascii_digit());
    let port: u16 = port.parse().ok().filter(|_| digits)?;
    (port != 0).then_some(port)
}
