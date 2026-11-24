//! One send through one channel (NOT-002, D-17, D-21); test and real notifications share it.

use super::{
    failure::Failure,
    outgoing::{Notice, Outgoing, outgoing},
};
use crate::fleet::{
    credentials::Vault,
    notifications::{
        Channel, DeliveryReason, InstallationSettings, NotificationKind,
        channels::Settings,
        egress::Egress,
        sealing,
        template::{self, Values},
    },
};

/// Every send leaves through the one egress path (P3-003).
#[derive(Clone)]
pub(in crate::fleet::notifications) struct Sender {
    vault: Vault,
    egress: Egress,
}

impl Sender {
    pub(in crate::fleet::notifications) const fn new(vault: Vault, egress: Egress) -> Self {
        Self { vault, egress }
    }

    /// Without both channel templates, `defaults` words the notice (NOT-007).
    pub(in crate::fleet::notifications) async fn send(
        &self,
        channel: &Channel,
        (kind, values): (NotificationKind, &Values),
        defaults: &InstallationSettings,
    ) -> Result<(), Failure> {
        let fields = &channel.fields;
        let settings = Settings::from_stored(fields.kind, &fields.settings)
            .map_err(|_| Failure::of(DeliveryReason::SetupFailed))?;
        let secrets = fields
            .secrets
            .as_deref()
            .map(|sealed| sealing::open(&self.vault, channel.id, &settings, sealed))
            .transpose()
            .map_err(|_| Failure::of(DeliveryReason::SecretUnreadable))?;
        let (title, body) = match (&fields.title_template, &fields.body_template) {
            (Some(title), Some(body)) => (title, body),
            _ => (&defaults.title_template, &defaults.body_template),
        };
        let rendered = if kind == NotificationKind::Reminder {
            template::given(values)
        } else {
            template::render(title, body, values)
                .map_err(|_| Failure::of(DeliveryReason::SetupFailed))?
        };
        let notice = Notice {
            kind,
            values,
            rendered: &rendered,
        };
        let request = outgoing(&settings, secrets, &notice)?;
        self.deliver(request).await
    }

    async fn deliver(&self, request: Outgoing) -> Result<(), Failure> {
        let sent = match request {
            Outgoing::Http(request) => self.egress.send_http(request).await,
            Outgoing::Mail {
                server,
                login,
                message,
            } => {
                self.egress
                    .send_mail(&server, login.as_ref(), &message)
                    .await
            }
        };
        sent.map_err(Failure::from)
    }
}
