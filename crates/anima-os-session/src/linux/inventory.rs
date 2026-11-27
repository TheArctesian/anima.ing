use super::{
    Error, Result,
    http::{graph, text},
};
use serde_json::{Value, json};
use std::{collections::HashSet, net::IpAddr};

pub async fn device(token: &str, name: &str) -> Result<String> {
    let linked = graph("query{linkDevices{name address}}", json!({}), Some(token)).await?;
    let matches: Vec<_> = linked["linkDevices"]
        .as_array()
        .ok_or(Error::Refused("Link inventory is unavailable."))?
        .iter()
        .filter(|entry| entry["name"] == name)
        .collect();
    if matches.len() != 1 {
        return Err(Error::Refused(
            "The hub could not uniquely identify this computer.",
        ));
    }
    let address: IpAddr = text(matches[0], "address")?
        .parse()
        .map_err(|_| Error::Refused("The private link address is invalid."))?;
    if let Some(id) = existing(token, address).await? {
        return Ok(id);
    }
    let sites = graph("query{sites{id name subnet}}", json!({}), Some(token)).await?;
    let site = sites["sites"].as_array().and_then(|sites| {
        sites
            .iter()
            .find(|site| site["subnet"] == "fd41:4e49:4d41::/48")
    });
    let site = if let Some(site) = site {
        text(site, "id")?.to_owned()
    } else {
        let created = graph(
            "mutation($name:String!,$subnet:Cidr!){createSite(name:$name,subnet:$subnet){id}}",
            json!({"name":"Link","subnet":"fd41:4e49:4d41::/48"}),
            Some(token),
        )
        .await?;
        text(&created["createSite"], "id")?.to_owned()
    };
    let answer = graph("mutation($input:CreateDeviceInput!){createDevice(input:$input){id}}",json!({"input":{
        "siteId":site,"name":std::fs::read_to_string("/proc/sys/kernel/hostname")?.trim(),"host":address.to_string(),
        "kind":"server","os":"LINUX","tags":[],"addDefaultCheck":false,
        "ssh":{"enabled":true,"port":22,"username":"admin","auth":"PASSWORD"},
        "rdp":{"enabled":true,"port":3389,"username":"admin","security":"TLS","ignoreCertificate":false,"audioOut":false},"vnc":{"enabled":true,"port":5900,"username":"admin"}}}),Some(token)).await?;
    Ok(text(&answer["createDevice"], "id")?.to_owned())
}

async fn existing(token: &str, address: IpAddr) -> Result<Option<String>> {
    let mut cursor = Value::Null;
    let mut seen = HashSet::new();
    for _ in 0..1000 {
        let answer = graph("query($after:String){devices(first:100,after:$after){edges{node{id host}} pageInfo{hasNextPage endCursor}}}",json!({"after":cursor}),Some(token)).await?;
        let batch = &answer["devices"];
        for edge in batch["edges"]
            .as_array()
            .ok_or(Error::Refused("Device inventory is incomplete."))?
        {
            let node = &edge["node"];
            if node["host"]
                .as_str()
                .and_then(|host| host.parse::<IpAddr>().ok())
                == Some(address)
            {
                return Ok(Some(text(node, "id")?.to_owned()));
            }
        }
        if batch["pageInfo"]["hasNextPage"] == false {
            return Ok(None);
        }
        let next = text(&batch["pageInfo"], "endCursor")?;
        if !seen.insert(next.to_owned()) {
            return Err(Error::Refused("Device inventory repeated a page."));
        }
        cursor = Value::String(next.into());
    }
    Err(Error::Refused("Device inventory exceeded its page limit."))
}
