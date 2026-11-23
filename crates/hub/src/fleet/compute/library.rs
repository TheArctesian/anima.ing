//! The model library (compute plan, LIBM-001 to LIBM-009, SEL-003).
//!
//! Writes take the caller's connection so the audit entry shares the transaction (LOG-005).
//! A repository is read through [`Fetch`] before the transaction starts.
//! Messages in [`Error`] hold no input value (LOG-007).

pub mod capability;
mod fetch;
pub mod graphql;
mod model;
mod naming;
mod publisher;
mod reading;
mod repository;
mod store;
#[cfg(test)]
mod tests;
mod variants;

use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::api_errors::{FieldError, field_names};

pub use capability::Capability;
pub use fetch::{Fetch, FetchError, HuggingFace};
pub use model::{Alias, Facts, Format, LayerShape, Missing, Model, Refusal, Variant, VariantFile};
pub use naming::address;
pub use publisher::{PublisherApproval, Widening};
pub use reading::Read;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the model is not in the library")]
    NotFound(Missing),
    #[error("input is not valid: {}", field_names(.0))]
    Validation(Vec<FieldError>),
    #[error("the repository is refused: {0}")]
    Refused(Refusal),
    #[error("the source of the models failed: {0}")]
    Source(#[from] FetchError),
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("a stored model cannot be read")]
    Unreadable,
}

fn invalid(error: FieldError) -> Error {
    Error::Validation(vec![error])
}

/// Every model in name order, with its variants and aliases (SEL-005).
///
/// # Errors
/// `Database` when a query fails.
pub async fn list(db: &PgPool) -> Result<Vec<Model>, Error> {
    let mut conn = db.acquire().await?;
    store::load(&mut conn, None).await
}

/// The model of this name or alias; a `name:tag` gives its model (LIBM-006, LIBM-007).
///
/// # Errors
/// `NotFound` when no model has the name or the alias; `Database` when a query fails.
pub async fn get(db: &PgPool, name: &str) -> Result<Model, Error> {
    let mut conn = db.acquire().await?;
    let (id, _) = store::resolve(&mut *conn, naming::model_part(name.trim()))
        .await?
        .ok_or(Error::NotFound(Missing::Model))?;
    store::load(&mut conn, Some(id))
        .await?
        .pop()
        .ok_or(Error::NotFound(Missing::Model))
}

/// A variant as a pull takes it (task L1-7): its identifier, the name of its model and the
/// variant with its files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PullSource {
    pub variant_id: Uuid,
    pub model: String,
    pub variant: Variant,
}

/// The variant `name:tag` that a pull takes (PULL-001); an alias names its model. A name with
/// no tag is refused with the tags of the model: the hub selects no variant until the runtime
/// choice of task L1-8 exists (PULL-002).
///
/// # Errors
/// `NotFound` when no model has the name; `Validation` on `model` for no tag or a tag that the
/// model does not have; `Database` when a query fails.
pub async fn pull_source(db: &PgPool, address: &str) -> Result<PullSource, Error> {
    let (name, tag) = address
        .trim()
        .split_once(':')
        .map_or((address.trim(), None), |(name, tag)| (name, Some(tag)));
    let model = get(db, name).await?;
    let tags: Vec<&str> = model
        .variants
        .iter()
        .map(|each| each.tag.as_str())
        .collect();
    let named = tag.and_then(|tag| model.variants.iter().find(|each| each.tag == tag));
    let Some(variant) = named else {
        let what = if tag.is_some() {
            "has no variant of that tag"
        } else {
            "needs a tag"
        };
        return Err(invalid(FieldError::new(
            "model",
            format!(
                "{} {what}: name one of {}:{}.",
                model.name,
                model.name,
                tags.join(", :")
            ),
        )));
    };
    let variant_id = store::variant_id(db, &model.name, &variant.tag)
        .await?
        .ok_or(Error::NotFound(Missing::Model))?;
    Ok(PullSource {
        variant_id,
        variant: variant.clone(),
        model: model.name,
    })
}

/// The approved organisations in name order (LIBM-003).
///
/// # Errors
/// `Database` when the query fails.
pub async fn sources(db: &PgPool) -> Result<Vec<String>, Error> {
    store::sources(db).await
}

/// Adds an organisation to the approved list; returns false when it was there already.
///
/// # Errors
/// `Validation` (`organisation`) when the name is not an organisation name.
pub async fn add_source(
    conn: &mut PgConnection,
    organisation: &str,
    now: DateTime<Utc>,
) -> Result<bool, Error> {
    let organisation = naming::organisation("organisation", organisation).map_err(invalid)?;
    store::add_source(conn, &organisation, now).await
}

/// Reads a repository for a model without storing it (LIBM-004); with no `name`, the hub
/// makes one from the repository name.
///
/// # Errors
/// `Validation`; `Refused` (source not approved or no variant); `Source` when it fails.
pub async fn read(
    db: &PgPool,
    fetch: &impl Fetch,
    repository: &str,
    name: Option<&str>,
) -> Result<Read, Error> {
    let source = naming::repository(repository).map_err(invalid)?;
    let name = match name {
        Some(text) => naming::name("name", text).map_err(invalid)?,
        None => naming::name("name", &naming::proposed(&source)).map_err(invalid)?,
    };
    match get(db, &name).await {
        Ok(model) if model.name != name => {
            return Err(invalid(FieldError::new(
                "name",
                "The name is an alias. Use a different name.",
            )));
        }
        Ok(_) | Err(Error::NotFound(_)) => {}
        Err(other) => return Err(other),
    }
    let approved = store::sources(db).await?;
    reading::read(fetch, &source, name, &approved).await
}

/// Stores what [`read`] gave; each variant replaces the stored variant of the same tag.
///
/// # Errors
/// `Database` when a write fails.
pub async fn save(
    conn: &mut PgConnection,
    read: &Read,
    now: DateTime<Utc>,
) -> Result<Model, Error> {
    let id = store::save_model(conn, &read.name, &read.facts, now).await?;
    for variant in &read.variants {
        store::save_variant(conn, id, variant).await?;
    }
    store::load(conn, Some(id))
        .await?
        .pop()
        .ok_or(Error::NotFound(Missing::Model))
}

/// Points an alias at a model; returns the name of the model it pointed at before (LIBM-007, SEL-003).
///
/// # Errors
/// `Validation` when the alias is not a valid name or names a model; `NotFound` for no model.
pub async fn set_alias(
    conn: &mut PgConnection,
    alias: &str,
    model: &str,
) -> Result<(Alias, Option<String>), Error> {
    let alias = naming::name("alias", alias).map_err(invalid)?;
    if let Some((_, name)) = store::resolve(&mut *conn, &alias).await?
        && name == alias
    {
        return Err(invalid(FieldError::new(
            "alias",
            "A model has this name. Use a different alias.",
        )));
    }
    let (id, name) = store::resolve(&mut *conn, naming::model_part(model.trim()))
        .await?
        .ok_or(Error::NotFound(Missing::Model))?;
    let before = store::set_alias(conn, &alias, id).await?;
    Ok((Alias { alias, model: name }, before))
}

/// The hub setting of the publisher approval (LIBM-003).
///
/// # Errors
/// `Database` when the query fails.
pub async fn publisher_approval(db: &PgPool) -> Result<PublisherApproval, Error> {
    store::publisher_approval(db).await
}

/// # Errors
/// `Database` when the write fails.
pub async fn set_publisher_approval(
    conn: &mut PgConnection,
    mode: PublisherApproval,
) -> Result<(), Error> {
    store::set_publisher_approval(conn, mode).await
}

/// Approves the publisher of a read model when the setting and the actor permit it (R-343).
/// `person` is false for an API key or an agent.
///
/// # Errors
/// `Database` when a query or the write fails.
pub async fn widen(
    conn: &mut PgConnection,
    read: &Read,
    person: bool,
    now: DateTime<Utc>,
) -> Result<Widening, Error> {
    let mode = store::publisher_approval(&mut *conn).await?;
    let approved = store::sources(&mut *conn).await?;
    let widening = publisher::widening(mode, person, read.facts.publisher.as_deref(), &approved);
    if let Widening::Add(organisation) = &widening {
        store::add_source(conn, organisation, now).await?;
    }
    Ok(widening)
}
