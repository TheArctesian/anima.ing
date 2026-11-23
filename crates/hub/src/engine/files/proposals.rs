//! Extraction proposals (K-E4 Section 10; VIS-001 to VIS-004).
//! Nothing of an extraction is a record until the operator confirms it, and a decided proposal does not change.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool};

use super::{Bad, Error, store};
use crate::{
    engine::{
        extensions::{Key, Sensitivity},
        records::Reference,
    },
    ids::{AttachmentId, ProposalId},
};

const MAX_FIELDS: usize = 100;
const MAX_VALUE: usize = 2000;
pub(super) const MAX_PROPOSALS: i64 = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    Text,
    Number,
    Date,
    Boolean,
    Reference,
}

/// A `None` confidence is unknown, not certain (VIS-004).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: FieldType,
    pub value: String,
    pub confidence: Option<f64>,
}

/// Where the extraction ran (VIS-002).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Client,
    Hub,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProposalState {
    Proposed,
    Confirmed,
    Rejected,
}

impl Origin {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Client => "client",
            Self::Hub => "hub",
        }
    }
}

impl ProposalState {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Confirmed => "confirmed",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Proposal {
    pub id: ProposalId,
    pub attachment: Option<AttachmentId>,
    pub owner: Reference,
    pub form: String,
    pub origin: Origin,
    /// As the extraction gave them while proposed; as the operator left them once confirmed.
    pub fields: Vec<Field>,
    pub state: ProposalState,
    pub created_at: DateTime<Utc>,
    pub decided_at: Option<DateTime<Utc>>,
}

/// The data class of `extension` decides where the extraction may run (VIS-002, VLT-001).
#[derive(Clone, Debug)]
pub struct NewProposal {
    pub owner: Reference,
    pub form: String,
    pub extension: Key,
    pub origin: Origin,
    pub attachment: Option<AttachmentId>,
    pub fields: Vec<Field>,
}

fn is_form(text: &str) -> bool {
    crate::engine::sealing_keys::is_name(text)
}

fn has_form(kind: FieldType, value: &str) -> bool {
    match kind {
        FieldType::Text => !value.chars().any(|c| c.is_control() && c != '\n'),
        FieldType::Number => value.parse::<f64>().is_ok_and(f64::is_finite),
        FieldType::Date => NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok(),
        FieldType::Boolean => matches!(value, "true" | "false"),
        FieldType::Reference => Reference::parse(value).is_ok(),
    }
}

/// # Errors
/// The bad fields, named `fields.<index>.<part>`.
pub fn checked_fields(fields: &[Field]) -> Result<(), Vec<Bad>> {
    let mut bad = Vec::new();
    if fields.len() > MAX_FIELDS {
        bad.push(Bad::new("fields", "Give at most 100 fields."));
    }
    for (index, field) in fields.iter().enumerate() {
        let named = |part: &str| format!("fields.{index}.{part}");
        if !is_form(&field.name) || fields[..index].iter().any(|f| f.name == field.name) {
            bad.push(Bad::new(
                named("name"),
                "Give a name of lower-case letters, digits or underscores, once.",
            ));
        }
        if field.value.chars().count() > MAX_VALUE || !has_form(field.kind, &field.value) {
            bad.push(Bad::new(
                named("value"),
                "Give a value of at most 2000 characters in the form of its type.",
            ));
        }
        if field.confidence.is_some_and(|c| !(0.0..=1.0).contains(&c)) {
            bad.push(Bad::new(
                named("confidence"),
                "Give a confidence from 0 to 1, or none.",
            ));
        }
    }
    if bad.is_empty() { Ok(()) } else { Err(bad) }
}

/// A Class A image never reaches the hub in a form it can read (VLT-001); a client proposal waits for client sealing (K-E5 Section 3, E0-6).
fn admitted(origin: Origin, class: Sensitivity) -> Result<(), Bad> {
    match (class, origin) {
        (Sensitivity::A, Origin::Hub) => Err(Bad::new(
            "origin",
            "This form is Class A: its extraction runs on the client only.",
        )),
        (Sensitivity::A, Origin::Client) => Err(Bad::new(
            "extension",
            "A Class A proposal must be sealed on the client; the hub does not take it yet.",
        )),
        _ => Ok(()),
    }
}

/// # Errors
/// `Invalid` for a bad form, bad fields, a refused origin or an attachment of another owner; `Database`.
pub async fn propose(
    conn: &mut PgConnection,
    new: NewProposal,
    at: DateTime<Utc>,
) -> Result<Proposal, Error> {
    let mut bad = checked_fields(&new.fields).err().unwrap_or_default();
    if !is_form(&new.form) {
        bad.push(Bad::new(
            "form",
            "Give a form in lower snake case, such as garment.",
        ));
    }
    bad.extend(admitted(new.origin, new.extension.entry().data_class).err());
    if let Some(id) = new.attachment {
        let found = store::attachment(&mut *conn, id).await?;
        if found.is_none_or(|attachment| attachment.owner != new.owner) {
            bad.push(Bad::new(
                "attachmentId",
                "Give an attachment of the same owner.",
            ));
        }
    }
    if !bad.is_empty() {
        return Err(Error::Invalid(bad));
    }
    let proposal = Proposal {
        id: uuid::Uuid::now_v7().into(),
        attachment: new.attachment,
        owner: new.owner,
        form: new.form,
        origin: new.origin,
        fields: new.fields,
        state: ProposalState::Proposed,
        created_at: at,
        decided_at: None,
    };
    store::insert_proposal(conn, &proposal).await?;
    Ok(proposal)
}

/// # Errors
/// `Invalid` for bad fields; `NotFound`; `Decided` for a proposal that is decided; `Database`.
pub async fn confirm(
    conn: &mut PgConnection,
    id: ProposalId,
    fields: Vec<Field>,
    (at, by): (DateTime<Utc>, &str),
) -> Result<Proposal, Error> {
    checked_fields(&fields).map_err(Error::Invalid)?;
    decided(
        conn,
        id,
        (ProposalState::Confirmed, Some(&fields)),
        (at, by),
    )
    .await
}

/// # Errors
/// `NotFound`; `Decided` for a proposal that is decided; `Database`.
pub async fn reject(
    conn: &mut PgConnection,
    id: ProposalId,
    (at, by): (DateTime<Utc>, &str),
) -> Result<Proposal, Error> {
    decided(conn, id, (ProposalState::Rejected, None), (at, by)).await
}

async fn decided(
    conn: &mut PgConnection,
    id: ProposalId,
    decision: (ProposalState, Option<&[Field]>),
    (at, by): (DateTime<Utc>, &str),
) -> Result<Proposal, Error> {
    if let Some(proposal) = store::decide(conn, id, decision, (at, by)).await? {
        return Ok(proposal);
    }
    match store::proposal(&mut *conn, id).await? {
        Some(_) => Err(Error::Decided),
        None => Err(Error::NotFound("proposal")),
    }
}

/// # Errors
/// `Database` when the read fails.
pub async fn confirmed(db: &PgPool, owner: &Reference) -> Result<Vec<Proposal>, Error> {
    Ok(store::proposals(
        db,
        Some(owner),
        Some(ProposalState::Confirmed),
        MAX_PROPOSALS,
    )
    .await?)
}

/// # Errors
/// `Database` when the read fails.
pub async fn list(
    db: &PgPool,
    owner: Option<&Reference>,
    state: Option<ProposalState>,
) -> Result<Vec<Proposal>, Error> {
    Ok(store::proposals(db, owner, state, MAX_PROPOSALS).await?)
}
