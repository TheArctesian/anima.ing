//! Connecting an account on the operator's own server with a key of that server (K-E1 Section 7b, R-261).

use source_exchange::Acquisition;
use uuid::Uuid;
use zeroize::Zeroizing;

use super::{
    Error, Scheduling,
    acquisition::{account_key, transport::Fault},
    connect::{Established, first_run, label},
    store::{self, Connecting},
    trail::connected,
};
use crate::{
    engine::extensions,
    fleet::{
        credentials::{Secret, store_source_secret},
        events::{self, Trail},
    },
    ids::SourceAccountId,
};

pub struct KeyConnection {
    pub source: &'static str,
    pub label: String,
    pub base: String,
    pub api_key: Zeroizing<String>,
}

/// The same base again replaces the stored key and keeps the account and its records.
///
/// # Errors
/// Bad input, a refused address or key, an unreachable server or a failed store; nothing is stored unless it returns `Ok`.
pub async fn connect_with_key(
    s: &Scheduling,
    trail: Trail<'_>,
    input: KeyConnection,
) -> Result<Established, Error> {
    let label = label(&input.label).ok_or(Error::Label)?;
    let declaration = *s
        .registry
        .declaration(input.source)
        .ok_or(Error::UnknownSource)?;
    let Acquisition::AccountKey(with_key) = declaration.acquisition else {
        return Err(Error::UnknownSource);
    };
    let base = account_key::base(&input.base).map_err(|_| Error::Base)?;
    let api_key = account_key::key(&input.api_key).map_err(|_| Error::ApiKey)?;
    let proof = Secret::new(api_key.as_bytes().to_vec());
    let answer = s
        .transport
        .get_at(&base, &(with_key.probe)(), &proof)
        .await
        .map_err(|fault| match fault {
            Fault::Refused => Error::AddressRefused,
            Fault::Network => Error::Unreachable,
        })?;
    drop(proof);
    match answer.status {
        200..=299 => {}
        401 | 403 => return Err(Error::ApiKeyRefused),
        _ => return Err(Error::Unreachable),
    }
    let instance_key = account_key::instance_key(&base);
    let mut tx = s.pool.begin().await?;
    let connecting = Connecting {
        id: SourceAccountId::from(Uuid::now_v7()),
        source: declaration.key,
        label: &label,
        instance_key: &instance_key,
        now: trail.now,
    };
    let (id, created) = store::connect(&mut tx, &connecting).await?;
    let secret = account_key::to_secret(&base, api_key);
    let credential = store_source_secret(&mut tx, &s.vault, id, secret).await?;
    store::set_credential(&mut tx, id, credential).await?;
    let named = (declaration.key, label.as_str(), instance_key.as_str());
    trail
        .write(&mut tx, connected(id, named, created))
        .await
        .map_err(|events::Error::Database(error)| Error::Database(error))?;
    tx.commit().await?;
    drop(input);
    s.feed.publish(id);
    let on = extensions::on_keys(&s.pool).await?;
    let consumer_on = s.registry.consumer_on(declaration.key, &on);
    if consumer_on {
        first_run(s.clone(), id);
    }
    Ok(Established {
        id,
        created,
        consumer_on,
    })
}
