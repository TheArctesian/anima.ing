//! Source capability belongs to the node owner. Only validated claims enter the journal.
use super::{
    Action, Coordinator, Error, MAX_REPLY_BYTES, MAX_REQUEST_BYTES, Request, SOCKET, Value,
};
use chrono::{DateTime, Utc};
use daemon_core::{
    source_jobs::private_checkout,
    wire::{
        BuildRecipes, Claim, Free, SOURCE_IMAGE_PRIVATE_CHECKOUT, Switch, WorkArchitecture,
        WorkJob, WorkKind, WorkOs, WorkReport, WorkType,
    },
    work::{
        checks::{self, Checked, Machine},
        permitted::Repositories,
        refusal::{self, Cause},
        types::Version,
    },
};

pub(crate) struct SourceOffer {
    pub(crate) claim: Claim,
    repositories: Repositories,
}

pub(crate) enum Selection {
    Job(Box<WorkJob>),
    Refused(Box<WorkReport>),
    Drop,
}

impl Coordinator {
    pub(crate) fn accept_claim(&mut self, job: &WorkJob) -> Result<(), Error> {
        self.journal
            .reserve(self.device, &self.origin, job)
            .map(drop)
    }

    pub(crate) async fn source_offer(&self) -> Result<Option<SourceOffer>, Error> {
        let holds = self.holds()?;
        if !holds.is_empty() || self.journal.full() {
            return Ok(None);
        }
        let request = Request {
            device: self.device,
            origin: self.origin.clone(),
            action: Action::Capacity,
        };
        let capacity = crate::runtime::owner_socket::call(
            SOCKET,
            &request,
            (MAX_REQUEST_BYTES, MAX_REPLY_BYTES),
        )
        .await
        .ok_or(Error::Unconfirmed)?;
        SourceOffer::new(self.device, &capacity).map(Some)
    }
}

impl SourceOffer {
    fn new(device: uuid::Uuid, capacity: &Value) -> Result<Self, Error> {
        let free: Free = serde_json::from_value(capacity["free"].clone())?;
        if capacity["enabled"] != true || capacity["available"] != true || free.slots != 1 {
            return Err(Error::Unconfirmed);
        }
        let repositories = capacity["repositories"]
            .as_str()
            .ok_or(Error::Unconfirmed)?;
        let repositories = Repositories::parse(repositories).map_err(|_| Error::Unconfirmed)?;
        let architecture = match std::env::consts::ARCH {
            "x86_64" => WorkArchitecture::X86_64,
            "aarch64" => WorkArchitecture::Aarch64,
            _ => return Err(Error::Unconfirmed),
        };
        let mut recipes = vec!["source_image_v1".into()];
        if capacity["privateCheckout"] == true {
            recipes.push(SOURCE_IMAGE_PRIVATE_CHECKOUT.into());
        }
        let build_recipes = BuildRecipes::try_from(recipes).map_err(|_| Error::Unconfirmed)?;
        Ok(Self {
            claim: Claim {
                build_recipes: Some(build_recipes),
                applications: Switch::Off,
                device,
                switch: Switch::On,
                types: vec![WorkType::Build],
                kinds: vec![WorkKind::Container],
                os: WorkOs::Linux,
                architecture,
                free,
                holds: Vec::new(),
                unclean: false,
            },
            repositories,
        })
    }

    pub(crate) fn select(&self, item: &Value, now: DateTime<Utc>) -> Selection {
        let machine = Machine {
            switch_on: true,
            policy_permits: true,
            kinds: &self.claim.kinds,
            isolation: &self.claim.kinds,
            unclean: false,
            repositories: &self.repositories,
            lowest_runner: Version(0, 0, 0),
        };
        match checks::check(item, &machine) {
            Checked::Start(job) if self.fits(&job, now) => Selection::Job(job),
            Checked::Start(job) => Selection::Refused(Box::new(refusal::report(
                job.id,
                job.attempt,
                Cause::Policy,
            ))),
            Checked::Refuse { id, attempt, cause } => {
                Selection::Refused(Box::new(refusal::report(id, attempt, cause)))
            }
            Checked::Drop => Selection::Drop,
        }
    }

    fn fits(&self, job: &WorkJob, now: DateTime<Utc>) -> bool {
        job.work_type == WorkType::Build
            && job.kind == WorkKind::Container
            && job.parameters["recipe"] == "source_image_v1"
            && job.parameters["target"] == format!("linux-{}", std::env::consts::ARCH)
            && (job.secrets.is_empty()
                || (private_checkout(job)
                    && self
                        .claim
                        .build_recipes
                        .as_ref()
                        .is_some_and(|recipes| recipes.supports(SOURCE_IMAGE_PRIVATE_CHECKOUT))))
            && job.lease.until > now
            && (512 * 1024 * 1024..=self.claim.free.memory_bytes).contains(&job.limits.memory_bytes)
            && (21 * 1024_u64.pow(3)..=self.claim.free.disk_bytes).contains(&job.limits.disk_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn source_offer_checks_owner_policy_budget_and_lease_before_adoption() {
        let mut capacity = json!({"enabled":true,"available":true,"repositories":"example/sample",
            "free":{"slots":1,"memoryBytes":1_073_741_824_u64,"diskBytes":24 * 1024_u64.pow(3)}});
        let device = uuid::Uuid::now_v7();
        let offer = SourceOffer::new(device, &capacity).unwrap();
        let now = Utc::now();
        let mut job = super::super::tests::job();
        job.lease.until = now + chrono::TimeDelta::seconds(60);
        job.parameters["target"] = json!(format!("linux-{}", std::env::consts::ARCH));
        let item = serde_json::to_value(&job).unwrap();
        assert!(matches!(offer.select(&item, now), Selection::Job(_)));

        let mut private = job.clone();
        private.project = Some(uuid::Uuid::now_v7());
        private.secrets = vec![daemon_core::wire::REPOSITORY_TOKEN.to_owned()];
        let private = serde_json::to_value(private).unwrap();
        assert!(matches!(offer.select(&private, now), Selection::Refused(_)));
        capacity["privateCheckout"] = json!(true);
        let private_offer = SourceOffer::new(device, &capacity).unwrap();
        assert!(matches!(
            private_offer.select(&private, now),
            Selection::Job(_)
        ));

        let mut outside = item.clone();
        outside["repository"]["owner"] = json!("other");
        let Selection::Refused(report) = offer.select(&outside, now) else {
            panic!("policy accepted");
        };
        assert_eq!(
            report.result.unwrap().cause.as_deref(),
            Some("repository_not_permitted")
        );
        let mut too_large = item.clone();
        too_large["limits"]["memoryBytes"] = json!(2 * 1024_u64.pow(3));
        assert!(matches!(
            offer.select(&too_large, now),
            Selection::Refused(_)
        ));
        assert!(matches!(
            offer.select(&item, job.lease.until),
            Selection::Refused(_)
        ));

        capacity["available"] = json!(false);
        assert!(SourceOffer::new(device, &capacity).is_err());
    }
}
