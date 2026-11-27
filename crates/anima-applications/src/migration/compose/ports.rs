use compose_rules::{Node, Variables};
use zeroize::Zeroizing;

use super::{Error, document::field, fields};

pub(super) fn rewrite(value: &mut Node, variables: &Variables<'_>) -> Result<(), Error> {
    let Node::List(ports) = value else {
        return Err(Error::ComposePort);
    };
    for port in ports {
        if let Node::Text(short) = port {
            *port = expand(short, variables)?;
        }
    }
    Ok(())
}

fn expand(short: &str, variables: &Variables<'_>) -> Result<Node, Error> {
    let (binding, protocol) = fields::first(short, '/').unwrap_or((short, "tcp"));
    let (host, target) = fields::last(binding, ':').ok_or(Error::ComposePort)?;
    let (address, published) = fields::last(host, ':')
        .map_or((None, host), |(address, published)| {
            (Some(address), published)
        });
    let mut port = vec![
        field("target", target),
        field("published", published),
        field("protocol", protocol),
    ];
    if let Some(address) = address {
        port.push(field("host_ip", unbracket(address, variables)?));
    }
    Ok(Node::Map(port))
}

fn unbracket<'a>(address: &'a str, variables: &Variables<'_>) -> Result<&'a str, Error> {
    if let Some(inner) = address
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
    {
        return Ok(inner);
    }
    let resolved = Zeroizing::new(variables.resolve(address)?);
    if resolved.contains(':') {
        return Err(Error::ComposePort);
    }
    Ok(address)
}
