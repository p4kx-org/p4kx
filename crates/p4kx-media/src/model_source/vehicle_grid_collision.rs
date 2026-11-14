// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::Serialize;

use super::origins::{self, ModelOrigins};
use super::payload::ModelSourceFamily;
use super::physics;
use super::vehicle_grid_record::LINK;
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{
    EntryInput, Identity, MediaError, MediaWriter, Reference, ReferenceResolution, SourceContext,
    Status, Subresource,
};

#[path = "vehicle_grid_proxy.rs"]
mod proxy;

use proxy::Proxy;

const TABLE: &str = "model-vehicle-grid-collision-table";
const SLOT: &str = "model-vehicle-grid-collision-slot";
const SCHEMA: &str = "ModelVehicleGridCollisionRecord";

pub(super) const SCHEMA_ROWS: [(&str, &str, usize, &str); 2] = [
    (TABLE, "vehicle-grid-collision-table", 3, SCHEMA),
    (SLOT, "vehicle-grid-collision-slot", 4, SCHEMA),
];

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum GridCollisionState {
    NoPath,
    TargetMissing,
    TargetAmbiguous,
    TargetNotSelected,
    TargetUnavailable,
    UnsupportedTarget,
    MalformedTarget,
    NoPhysicsTables,
    TablesPresent,
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum VehiclePlacement {
    Unavailable,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct GridCollisionTable {
    table: Subresource,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct GridCollisionSlot {
    slot: Subresource,
    model_frame: Subresource,
    vehicle_placement: VehiclePlacement,
}

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
enum GridCollisionRecord {
    VehicleGridCollisionTable(GridCollisionTable),
    VehicleGridCollisionSlot(GridCollisionSlot),
}

pub(super) fn insert_schema(base: &mut serde_json::Value) -> Result<(), MediaError> {
    crate::schema_merge::insert_payload(
        base,
        SCHEMA,
        crate::schema_merge::serialized_payload::<GridCollisionRecord>(),
    )
}

#[derive(Default)]
pub(super) struct ProxyCache {
    selected: HashMap<usize, Proxy>,
}

pub(super) struct CollisionProjection<'cache>(Selection<'cache>);

enum Selection<'cache> {
    Unread(GridCollisionState),
    Read(&'cache Proxy),
}

impl ProxyCache {
    pub(super) fn projection(
        &mut self,
        context: &SourceContext<'_>,
        path: Option<&Reference>,
        occurrence: [u64; 2],
        writer: &mut MediaWriter<'_>,
    ) -> Result<CollisionProjection<'_>, MediaError> {
        let input = match proxy_token(context, path)? {
            Ok(input) => input,
            Err(state) => return Ok(CollisionProjection(Selection::Unread(state))),
        };
        let position = input.position;
        if !self.selected.contains_key(&position) {
            self.selected.try_reserve(1).map_err(|cause| {
                MediaError::Contract(format!(
                    "cannot retain vehicle grid proxy inventory: {cause}"
                ))
            })?;
            let use_at = Subresource::new(LINK, occurrence.to_vec());
            let inventory = context.with_required_input(&input, &use_at, writer, proxy::read)?;
            self.selected.insert(position, inventory);
        }
        let proxy = self.selected.get(&position).ok_or_else(|| {
            MediaError::Contract("selected vehicle grid proxy was not inventoried".into())
        })?;
        Ok(CollisionProjection(Selection::Read(proxy)))
    }
}

fn proxy_token<'a>(
    context: &'a SourceContext<'_>,
    path: Option<&Reference>,
) -> Result<Result<EntryInput<'a>, GridCollisionState>, MediaError> {
    let target = match path.map(Reference::resolution) {
        None => return Ok(Err(GridCollisionState::NoPath)),
        Some(ReferenceResolution::Missing) => return Ok(Err(GridCollisionState::TargetMissing)),
        Some(ReferenceResolution::Ambiguous { .. }) => {
            return Ok(Err(GridCollisionState::TargetAmbiguous));
        }
        Some(ReferenceResolution::NotSelected { .. }) => {
            return Ok(Err(GridCollisionState::TargetNotSelected));
        }
        Some(ReferenceResolution::Unavailable { .. }) => {
            return Ok(Err(GridCollisionState::TargetUnavailable));
        }
        Some(ReferenceResolution::Resolved { target }) => target,
    };
    let directory = context.directory;
    let position = match &target.source {
        Identity::Entry { containers, entry }
            if &target.root == directory.root && containers == directory.chain =>
        {
            if ModelSourceFamily::of(&entry.path).is_none() {
                return Ok(Err(GridCollisionState::UnsupportedTarget));
            }
            entry.entry_index
        }
        _ => {
            return Err(MediaError::Contract(
                "vehicle grid proxy is outside its owning Archive".into(),
            ));
        }
    };
    Ok(Ok(EntryInput {
        root: directory.root,
        chain: directory.chain,
        position,
    }))
}

impl CollisionProjection<'_> {
    pub(super) const fn state(&self) -> GridCollisionState {
        match &self.0 {
            Selection::Unread(state) => *state,
            Selection::Read(proxy) => proxy.state,
        }
    }

    pub(super) fn state_evidence(
        &self,
        path: &OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        match &self.0 {
            Selection::Unread(_) => Ok(path.clone()),
            Selection::Read(proxy) => merge(path, &proxy.evidence, writer),
        }
    }

    pub(super) fn write_links(
        &self,
        occurrence: [u64; 2],
        originals: &ModelOrigins<'_>,
        path: &OriginContributions,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let Selection::Read(proxy) = &self.0 else {
            return Ok(());
        };
        for table in &proxy.tables {
            writer.checkpoint()?;
            let [record, component] = occurrence;
            let chunk = table.ordinal;
            let evidence = merge(path, &table.evidence, writer)?;
            let mut prepared = originals.prepared()?;
            origins::at(
                &mut prepared,
                "/subresource/indices",
                evidence.clone(),
                writer,
            )?;
            reference_origins(
                &mut prepared,
                "/data/payload/table",
                &evidence,
                originals,
                writer,
            )?;
            prepared.write(
                writer,
                &Subresource::new(TABLE, vec![record, component, chunk]),
                Status::Complete,
                &GridCollisionRecord::VehicleGridCollisionTable(GridCollisionTable {
                    table: Subresource::new(physics::TABLE, vec![chunk]),
                }),
                &[],
            )?;
            for slot in &table.slots {
                writer.checkpoint()?;
                write_slot(
                    [record, component, chunk, slot.ordinal],
                    slot,
                    &evidence,
                    originals,
                    writer,
                )?;
            }
        }
        Ok(())
    }
}

fn write_slot(
    indices: [u64; 4],
    slot: &proxy::Slot,
    table: &OriginContributions,
    originals: &ModelOrigins<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let evidence = merge(table, &slot.evidence, writer)?;
    let mut prepared = originals.prepared()?;
    origins::at(
        &mut prepared,
        "/subresource/indices",
        evidence.clone(),
        writer,
    )?;
    for field in ["/data/payload/slot", "/data/payload/model_frame"] {
        reference_origins(&mut prepared, field, &evidence, originals, writer)?;
    }
    let absent = OriginContributions::unknown(&originals.input.owner);
    origins::at(
        &mut prepared,
        "/data/payload/vehicle_placement",
        absent,
        writer,
    )?;
    let [_, _, chunk, slot] = indices;
    prepared.write(
        writer,
        &Subresource::new(SLOT, indices.to_vec()),
        Status::Complete,
        &GridCollisionRecord::VehicleGridCollisionSlot(GridCollisionSlot {
            slot: Subresource::new(physics::SLOT, vec![chunk, slot]),
            model_frame: Subresource::new(physics::FRAME, vec![chunk, slot]),
            vehicle_placement: VehiclePlacement::Unavailable,
        }),
        &[],
    )
}

fn reference_origins(
    prepared: &mut PreparedRecordOrigins,
    field: &str,
    evidence: &OriginContributions,
    originals: &ModelOrigins<'_>,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    origins::at(prepared, field, evidence.clone(), writer)?;
    origins::at(
        prepared,
        &format!("{field}/namespace"),
        originals.independent(),
        writer,
    )
}

fn merge(
    first: &OriginContributions,
    second: &OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let mut combined = writer.capture_origins();
    combined.include(first, writer)?;
    combined.include(second, writer)?;
    Ok(combined.finish())
}
