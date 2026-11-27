//! File one claim per device with every locally held host and source attempt.

use std::{sync::Arc, time::Duration};

use chrono::Utc;
use daemon_core::wire::{Claim, Free, Switch, WorkArchitecture, WorkKind, WorkOs, WorkType};
use serde_json::Value;

use crate::{
    hub::answer::Answer,
    source_work::{Error, Selection, SourceOffer},
};

use super::{Coordinator, HubTarget, credential, hosting::Hosting};

pub(super) async fn poll(
    owner: &mut Coordinator,
    target: &HubTarget,
    mut hosting: Option<&mut Hosting>,
) -> Result<(), Error> {
    let Some(credentials) = credential(target).await else {
        return Ok(());
    };
    let token = Arc::new(credentials.token);
    let host_holds = hosting
        .as_ref()
        .map_or_else(Vec::new, |hosting| hosting.holds());
    if let Some(hosting) = hosting.as_mut() {
        hosting.tick(target, &token);
    }
    let (offer, supported) = tokio::join!(source_offer(owner), host_available(hosting.as_deref()));
    let Some(mut claim) = offer.as_ref().map(|offer| offer.claim.clone()).or_else(|| {
        (!supported.is_empty())
            .then(|| empty_claim(target))
            .flatten()
    }) else {
        return Ok(());
    };
    claim.holds = owner.holds()?;
    if !claim.holds.is_empty() {
        claim.switch = Switch::On;
    }
    claim.holds.extend(host_holds);
    if !supported.is_empty() {
        claim.applications = Switch::On;
        claim.types.extend(&supported);
        claim.kinds.push(WorkKind::Host);
    }
    let Answer::Accepted(Some(item)) = target.hub.claim_work(&token, &claim).await else {
        return Ok(());
    };
    if item["kind"] == "host" {
        let kind = item["type"].as_str().and_then(WorkType::parse);
        if let Some(hosting) =
            hosting.filter(|_| kind.is_some_and(|kind| supported.contains(&kind)))
        {
            hosting.submit(target, token, item);
        }
    } else if let Some(offer) = offer {
        select(owner, target, (&token, &item), &offer).await;
    }
    Ok(())
}

async fn source_offer(owner: &Coordinator) -> Option<SourceOffer> {
    tokio::time::timeout(Duration::from_secs(3), owner.source_offer())
        .await
        .ok()?
        .ok()?
}

async fn host_available(hosting: Option<&Hosting>) -> Vec<WorkType> {
    match hosting {
        Some(hosting) => hosting.available().await,
        None => Vec::new(),
    }
}

fn empty_claim(target: &HubTarget) -> Option<Claim> {
    let architecture = match std::env::consts::ARCH {
        "x86_64" => WorkArchitecture::X86_64,
        "aarch64" => WorkArchitecture::Aarch64,
        _ => return None,
    };
    Some(Claim {
        build_recipes: None,
        device: target.device,
        switch: Switch::Off,
        applications: Switch::Off,
        types: Vec::new(),
        kinds: Vec::new(),
        os: WorkOs::Linux,
        architecture,
        free: Free {
            slots: 0,
            memory_bytes: 0,
            disk_bytes: 0,
        },
        holds: Vec::new(),
        unclean: false,
    })
}

async fn select(
    owner: &mut Coordinator,
    target: &HubTarget,
    (token, item): (&daemon_core::wire::DaemonToken, &Value),
    offer: &SourceOffer,
) {
    match offer.select(item, Utc::now()) {
        Selection::Job(job) => {
            if owner.accept_claim(&job).is_err() {
                tracing::warn!("Could not persist the claimed source attempt.");
            }
        }
        Selection::Refused(report) => {
            if let Ok(body) = serde_json::to_value(report) {
                let _ = target.hub.report_work(token, &body).await;
            }
        }
        Selection::Drop => (),
    }
}
