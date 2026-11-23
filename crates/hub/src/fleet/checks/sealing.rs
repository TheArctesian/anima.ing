//! SEC-003, SEC-005, SEC-010.

use super::{
    Error,
    config::CheckConfig,
    model::{Definition, FieldError, Sealer},
    rules::{self, Prepared, Sealable, Stored},
    store,
};
use crate::ids::CheckId;

/// The seal binds the secrets to the check and its configuration, SNMP secrets also to the
/// sealer's host.
pub(super) fn sealed(
    sealer: Sealer<'_>,
    id: CheckId,
    prepared: &Prepared,
) -> Result<Option<Vec<u8>>, Error> {
    match (&prepared.secrets, &prepared.config) {
        (Some(Sealable::Http(secrets)), CheckConfig::Http(config)) => {
            store::seal_http(sealer.vault, id, config, secrets).map(Some)
        }
        (Some(Sealable::Snmp(secrets)), CheckConfig::Snmp(config)) => {
            store::seal_snmp(sealer.vault, id, (config, sealer.host), secrets).map(Some)
        }
        _ => Ok(None),
    }
}

fn unopened() -> FieldError {
    FieldError::new(
        "config",
        "The stored secrets cannot be opened. Enter every secret again.",
    )
}

/// SNMP secrets sealed for another device host are not kept; each one left out is asked for
/// again (SEC-010).
pub(super) fn prepare_change(
    sealer: Sealer<'_>,
    id: CheckId,
    current: store::Current,
    definition: Definition,
) -> Result<Prepared, FieldError> {
    if definition.kind().as_sql() != current.kind {
        return Err(FieldError::new(
            "config",
            "A check keeps its type. Delete it and add a new check instead.",
        ));
    }
    let stored = match &definition {
        Definition::Http(input) if rules::needs_stored(input) => current
            .http(sealer.vault, id)
            .map_err(|_| unopened())?
            .map_or(Stored::Nothing, Stored::Http),
        Definition::Snmp(input) if rules::snmp_needs_stored(input) => {
            match current.snmp(sealer.vault, id, sealer.host) {
                Ok(stored) => stored.map_or(Stored::Nothing, Stored::Snmp),
                Err(Error::SecretsForOtherHost) => Stored::Nothing,
                Err(_) => return Err(unopened()),
            }
        }
        _ => Stored::Nothing,
    };
    rules::prepare(definition, stored)
}
