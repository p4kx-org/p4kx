// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::query::QuerySource;
use p4kx_extraction::{Record, SourceUseRole, Value as ForgeValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::character_payload::optional_reference;
use super::origins::{self, ModelOrigins};
use super::path_reference;
use super::vehicle_grid_collision::{GridCollisionState, ProxyCache};
use super::vehicle_grid_origins::{path_evidence, record_field};
use crate::origin_contributions::OriginContributions;

use crate::{
    MediaError, MediaIssue, MediaWriter, Reference, Severity, SourceContext, Status, Subresource,
};

const OWNER: &str = "model-vehicle-grid-database";
pub(super) const LINK: &str = "model-vehicle-grid";
const FAILURE: &str = "model-vehicle-grid-record-failure";
const SCHEMA_NAME: &str = "ModelVehicleGridRecord";
pub(super) const SCHEMA_ROWS: [(&str, &str, usize, &str); 3] = [
    (OWNER, "vehicle-grid-database", 0, SCHEMA_NAME),
    (LINK, "vehicle-grid", 2, SCHEMA_NAME),
    (FAILURE, "vehicle-grid-record-failure", 1, SCHEMA_NAME),
];

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum GridDatabaseState {
    Ready,
    AbsentSchema,
    UnsupportedSchema,
    InvalidDatabase,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct VehicleGrid<'record> {
    guid: String,
    record_file: Option<&'record str>,
    field: &'static str,
    state: GridPathDisposition,
    #[schemars(schema_with = "optional_reference")]
    path: Option<&'record Reference>,
    collision_state: GridCollisionState,
}

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
enum GridRecord<'record> {
    #[serde(rename = "vehicle-grid-database")]
    Database(GridDatabaseState),
    #[serde(rename = "vehicle-grid")]
    Occurrence(VehicleGrid<'record>),
    #[serde(rename = "vehicle-grid-record-failure")]
    RecordFailure(VehicleGridRecordFailure),
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct VehicleGridRecordFailure {
    reason: &'static str,
}

pub(super) fn insert_schema(base: &mut serde_json::Value) -> Result<(), MediaError> {
    crate::schema_merge::insert_payload(
        base,
        SCHEMA_NAME,
        crate::schema_merge::serialized_payload::<GridRecord<'_>>(),
    )
}

pub(super) fn summary(
    state: GridDatabaseState,
    length: usize,
    originals: &ModelOrigins<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = originals.prepared()?;
    let evidence = originals.payload(
        0..length as u64,
        "vehicleGridDatabase",
        SourceUseRole::Selector,
        writer,
    )?;
    origins::at(&mut prepared, "/data/payload", evidence.clone(), writer)?;
    let issue = match state {
        GridDatabaseState::Ready | GridDatabaseState::AbsentSchema => None,
        GridDatabaseState::UnsupportedSchema => {
            Some("the DataForge layout of the vehicle grid path is unsupported")
        }
        GridDatabaseState::InvalidDatabase => {
            Some("the selected DataForge input cannot be decoded")
        }
    };
    let issues = issue.map(|detail| {
        MediaIssue::new(
            "model.vehicle-grid-database",
            "entry",
            Severity::Warning,
            detail,
        )
    });
    let status = if issues.is_some() {
        origins::at(&mut prepared, "/status", evidence.clone(), writer)?;
        origins::at(&mut prepared, "/issues", evidence, writer)?;
        Status::Partial
    } else {
        Status::Complete
    };
    prepared.write(
        writer,
        &Subresource::new(OWNER, Vec::new()),
        status,
        &GridRecord::Database(state),
        issues.as_slice(),
    )
}

pub(super) fn row_failure(
    record: Record<'_>,
    detail: &'static str,
    originals: &ModelOrigins<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let id = u64::from(record.index());
    let proof = record_field(originals, id, "record", writer)?;
    let mut prepared = originals.prepared()?;
    for path in [
        "/subresource/indices",
        "/status",
        "/issues",
        "/data/payload",
    ] {
        origins::at(&mut prepared, path, proof.clone(), writer)?;
    }
    prepared.write(
        writer,
        &Subresource::new(FAILURE, vec![id]),
        Status::Partial,
        &GridRecord::RecordFailure(VehicleGridRecordFailure { reason: detail }),
        &[MediaIssue::new(
            "model.vehicle-grid-record",
            "record",
            Severity::Warning,
            detail,
        )],
    )
}

#[derive(Clone, Copy)]
pub(super) struct GridOccurrence<'database, 'sites> {
    pub(super) record: Record<'database>,
    pub(super) component_index: u32,
    pub(super) field: &'static str,
    pub(super) state: PathState<'database>,
    pub(super) sites: &'sites [QuerySource],
}

#[derive(Clone, Copy)]
pub(super) enum PathState<'database> {
    Present(&'database str),
    Null,
    Unreadable,
}

impl<'database> PathState<'database> {
    pub(super) const fn from_value(value: &ForgeValue<'database>) -> Option<Self> {
        match value {
            ForgeValue::String(value) => Some(Self::Present(value)),
            ForgeValue::Null => Some(Self::Null),
            ForgeValue::Error(_) => Some(Self::Unreadable),
            _ => None,
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum GridPathDisposition {
    Present,
    Null,
    Unreadable,
}

pub(super) fn occurrence(
    data: GridOccurrence<'_, '_>,
    context: &SourceContext<'_>,
    originals: &ModelOrigins<'_>,
    proxies: &mut ProxyCache,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let GridOccurrence {
        record,
        component_index,
        field,
        state,
        sites,
    } = data;
    let record_index = u64::from(record.index());
    let mut prepared = originals.prepared()?;
    for (path, field) in [
        ("/data/payload/guid", "guid"),
        ("/data/payload/record_file", "fileName"),
    ] {
        let original = record_field(originals, record_index, field, writer)?;
        origins::at(&mut prepared, path, original, writer)?;
    }
    let evidence = path_evidence(originals, record_index, field, sites, writer)?;
    origins::at(
        &mut prepared,
        "/subresource/indices",
        evidence.clone(),
        writer,
    )?;
    origins::at(
        &mut prepared,
        "/data/payload/field",
        originals.independent(),
        writer,
    )?;
    let occurrence = [record_index, u64::from(component_index)];
    let (state, path, path_origin, issues) =
        path_and_status(state, context, &evidence, &mut prepared, writer)?;
    let collision = proxies.projection(context, path.as_ref(), occurrence, writer)?;
    origins::at(
        &mut prepared,
        "/data/payload/collision_state",
        collision.state_evidence(&path_origin, writer)?,
        writer,
    )?;
    prepared.write(
        writer,
        &Subresource::new(LINK, occurrence.to_vec()),
        if issues.is_some() {
            Status::Partial
        } else {
            Status::Complete
        },
        &GridRecord::Occurrence(VehicleGrid {
            guid: record.guid().to_string(),
            record_file: record.stored_file_name().ok(),
            field,
            state,
            path: path.as_ref(),
            collision_state: collision.state(),
        }),
        issues.as_slice(),
    )?;
    collision.write_links(occurrence, originals, &path_origin, writer)
}

type GridPath = (
    GridPathDisposition,
    Option<Reference>,
    OriginContributions,
    Option<MediaIssue>,
);

fn path_and_status(
    state: PathState<'_>,
    context: &SourceContext<'_>,
    evidence: &OriginContributions,
    prepared: &mut crate::origin_prepared::PreparedRecordOrigins,
    writer: &mut MediaWriter<'_>,
) -> Result<GridPath, MediaError> {
    origins::at(prepared, "/data/payload/state", evidence.clone(), writer)?;
    let (path, path_origin) = grid_reference(state, context, evidence, prepared, writer)?;
    let (state, issue) = match state {
        PathState::Present(_) => (GridPathDisposition::Present, None),
        PathState::Null => (GridPathDisposition::Null, None),
        PathState::Unreadable => {
            for pointer in ["/status", "/issues"] {
                origins::at(prepared, pointer, evidence.clone(), writer)?;
            }
            let issue = MediaIssue::new(
                "model.vehicle-grid-path",
                "path",
                Severity::Warning,
                "the vehicle component has an unreadable physics-grid path",
            );
            (GridPathDisposition::Unreadable, Some(issue))
        }
    };
    Ok((state, path, path_origin, issue))
}

fn grid_reference(
    state: PathState<'_>,
    context: &SourceContext<'_>,
    evidence: &OriginContributions,
    prepared: &mut crate::origin_prepared::PreparedRecordOrigins,
    writer: &mut MediaWriter<'_>,
) -> Result<(Option<Reference>, OriginContributions), MediaError> {
    let field = "/data/payload/path";
    let PathState::Present(path) = state else {
        origins::at(prepared, field, evidence.clone(), writer)?;
        return Ok((None, evidence.clone()));
    };
    let observed = path_reference::lookup(context.directory, path, writer)?;
    let selected = observed.write(field, evidence, evidence, prepared, writer)?;
    Ok((Some(observed.reference), selected))
}
