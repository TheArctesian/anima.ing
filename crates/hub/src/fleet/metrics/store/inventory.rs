//! Each daemon batch that brings an inventory replaces the stored one whole (CAP-020, CAP-029).

use chrono::{DateTime, Utc};
use daemon_core::inventory::{
    Architecture, Inventory, Os, Reason, Tool, ToolName, Verdict, mobile::MobileTools,
};
use sqlx::{PgConnection, PgPool};

use super::record::device_missing;
use crate::{
    fleet::metrics::{Error, capabilities::Capabilities},
    ids::DeviceId,
};

/// One statement, so each inventory's tools and targets are read together.
pub(in crate::fleet::metrics) async fn inventories(
    db: &PgPool,
) -> Result<Vec<Capabilities>, Error> {
    let rows: Vec<InventoryRow> = sqlx::query_as(
        "SELECT i.device_id, i.architecture, i.os, i.os_version, i.found_at, i.mobile_tools, \
           COALESCE((SELECT array_agg(t.name ORDER BY t.name COLLATE \"C\") \
                     FROM metrics.tools t WHERE t.device_id = i.device_id), '{}') AS tool_names, \
           COALESCE((SELECT array_agg(t.version ORDER BY t.name COLLATE \"C\") \
                     FROM metrics.tools t WHERE t.device_id = i.device_id), '{}') AS tool_versions, \
           COALESCE((SELECT array_agg(t.reason ORDER BY t.name COLLATE \"C\") \
                     FROM metrics.tools t WHERE t.device_id = i.device_id), '{}') AS tool_reasons, \
           COALESCE((SELECT array_agg(b.target ORDER BY b.target COLLATE \"C\") \
                     FROM metrics.build_targets b WHERE b.device_id = i.device_id), '{}') \
             AS build_targets \
         FROM metrics.inventories i",
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(Capabilities::try_from).collect()
}

pub(in crate::fleet::metrics) async fn write_inventory(
    conn: &mut PgConnection,
    device: DeviceId,
    inventory: &Inventory,
    found_at: DateTime<Utc>,
) -> Result<(), Error> {
    sqlx::query("DELETE FROM metrics.inventories WHERE device_id = $1")
        .bind(device)
        .execute(&mut *conn)
        .await?;
    sqlx::query(
        "INSERT INTO metrics.inventories (device_id, architecture, os, os_version, found_at, mobile_tools) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(device)
    .bind(inventory.architecture.as_str())
    .bind(inventory.os.as_str())
    .bind(&inventory.os_version)
    .bind(found_at)
    .bind(inventory.mobile_tools.as_ref().map(sqlx::types::Json))
    .execute(&mut *conn)
    .await
    .map_err(device_missing)?;
    let names: Vec<&str> = inventory
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect();
    let versions: Vec<Option<&str>> = inventory.tools.iter().map(Tool::version).collect();
    let reasons: Vec<Option<&str>> = inventory
        .tools
        .iter()
        .map(|tool| tool.reason().map(Reason::as_str))
        .collect();
    sqlx::query(
        "INSERT INTO metrics.tools (device_id, name, version, reason) \
         SELECT $1, name, version, reason \
         FROM UNNEST($2::text[], $3::text[], $4::text[]) AS t (name, version, reason)",
    )
    .bind(device)
    .bind(&names)
    .bind(&versions)
    .bind(&reasons)
    .execute(&mut *conn)
    .await?;
    sqlx::query(
        "INSERT INTO metrics.build_targets (device_id, target) \
         SELECT $1, target FROM UNNEST($2::text[]) AS b (target)",
    )
    .bind(device)
    .bind(&inventory.build_targets)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
pub(super) struct InventoryRow {
    device_id: DeviceId,
    architecture: String,
    os: String,
    os_version: String,
    found_at: DateTime<Utc>,
    tool_names: Vec<String>,
    tool_versions: Vec<Option<String>>,
    tool_reasons: Vec<Option<String>>,
    build_targets: Vec<String>,
    mobile_tools: Option<sqlx::types::Json<MobileTools>>,
}

impl TryFrom<InventoryRow> for Capabilities {
    type Error = Error;

    /// An unknown tool name is left out; an unknown reason reads as `failed`.
    fn try_from(row: InventoryRow) -> Result<Self, Error> {
        let mut tools: Vec<Tool> = row
            .tool_names
            .iter()
            .zip(row.tool_versions)
            .zip(row.tool_reasons)
            .filter_map(|((name, version), reason)| {
                let verdict = match version {
                    Some(version) => Verdict::Usable(version),
                    None => Verdict::NotUsable(Reason::parse(reason.as_deref().unwrap_or(""))),
                };
                Some(Tool {
                    name: ToolName::parse(name)?,
                    verdict,
                })
            })
            .collect();
        tools.sort_by_key(|tool| tool.name);
        let inventory = Inventory {
            remote_access: None,
            mobile_tools: row.mobile_tools.map(|snapshot| snapshot.0),
            architecture: Architecture::parse(&row.architecture).ok_or(Error::Unreadable)?,
            os: Os::parse(&row.os).ok_or(Error::Unreadable)?,
            os_version: row.os_version,
            tools,
            build_targets: row.build_targets,
        };
        Ok(Self {
            device: row.device_id,
            inventory,
            found_at: row.found_at,
            hardware: None,
        })
    }
}
