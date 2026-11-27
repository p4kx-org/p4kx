// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashSet};
use std::iter::Enumerate;
use std::slice::Iter;

use p4kx_extraction::{
    CancelToken, DataForge, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use super::crafter_definition::PreparedCrafter;
use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_query_session::checkpoint;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::loadout_position::{self, LoadoutPosition};
use crate::contract_loadout::Loadout;
use crate::guid_text::CatalogGuid;
use crate::loadout::alignment::AttachmentIdentity;
use crate::loadout::join::{JoinPort, LoadoutJoin};
use crate::loadout::staging::StagedLoadouts;
use crate::record_header_field::RecordHeaderField;
use crate::scan::CRAFTER_STRUCT;
use crate::schema_fields::every_field_required;

/// A physical host ship and crafter linked by the ship's resolved default Loadout.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct CrafterHost {
    ship_guid: SourcedValue<CatalogGuid>,
    ship_record_index: SourcedValue<u32>,
    crafter: AttachmentIdentity,
    mount_positions: Vec<SourcedValue<LoadoutPosition>>,
}

impl CrafterHost {
    /// Borrows the original ship GUID.
    #[must_use]
    pub const fn ship_guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.ship_guid
    }

    /// Borrows the exact ship and Loadout owner Record index.
    #[must_use]
    pub const fn ship_record_index(&self) -> &SourcedValue<u32> {
        &self.ship_record_index
    }

    /// Borrows the crafter's resolved physical identity.
    #[must_use]
    pub const fn crafter(&self) -> &AttachmentIdentity {
        &self.crafter
    }

    /// Borrows every reached mount in original Loadout traversal order.
    #[must_use]
    pub fn mount_positions(&self) -> &[SourcedValue<LoadoutPosition>] {
        &self.mount_positions
    }
}

pub(crate) fn table(
    forge: &DataForge<'_>,
    stations: &[PreparedCrafter<'_>],
    loadouts: &StagedLoadouts,
    origins: &CatalogOrigins,
    cancel: Option<&CancelToken>,
) -> Result<CatalogTable<CrafterHost>, CatalogError> {
    let stations: HashSet<_> = stations
        .iter()
        .filter_map(|station| {
            station
                .definition()
                .source()
                .record_index()
                .value()
                .copied()
        })
        .collect();
    let mut rows = Vec::new();
    if !stations.is_empty() {
        for ship in loadouts.ships() {
            rows.extend(project_join(&ship.view(), origins, cancel, |record| {
                Ok(stations.contains(&record))
            })?);
        }
    }
    rows.sort_by_key(|row| {
        (
            row.ship_record_index.value().copied(),
            row.crafter.target_record_index().value().copied(),
        )
    });
    Ok(CatalogTable::new(forge, &[CRAFTER_STRUCT], rows))
}

/// Projects the proved crafter mounts reached by one original ship Loadout.
pub(crate) fn project_loadout(
    loadout: &Loadout,
    origins: &CatalogOrigins,
    cancel: Option<&CancelToken>,
    is_crafter: impl FnMut(u32) -> Result<bool, CatalogError>,
) -> Result<Vec<CrafterHost>, CatalogError> {
    project_join(&LoadoutJoin::from(loadout), origins, cancel, is_crafter)
}

fn project_join(
    loadout: &LoadoutJoin<'_, impl JoinPort>,
    origins: &CatalogOrigins,
    cancel: Option<&CancelToken>,
    mut is_crafter: impl FnMut(u32) -> Result<bool, CatalogError>,
) -> Result<Vec<CrafterHost>, CatalogError> {
    let mut projection = HostProjection {
        groups: BTreeMap::new(),
        origins,
        cancel,
    };
    projection.read_loadout(loadout, &mut is_crafter)?;
    Ok(projection.groups.into_values().collect())
}

struct HostProjection<'a> {
    groups: BTreeMap<u32, CrafterHost>,
    origins: &'a CatalogOrigins,
    cancel: Option<&'a CancelToken>,
}

impl HostProjection<'_> {
    fn read_loadout(
        &mut self,
        loadout: &LoadoutJoin<'_, impl JoinPort>,
        is_crafter: &mut impl FnMut(u32) -> Result<bool, CatalogError>,
    ) -> Result<(), CatalogError> {
        let root = loadout.record_index;
        let root_use = self
            .origins
            .record(root)
            .observe_header(RecordHeaderField::RecordIndex, SourceUseRole::Selector)?;
        let mut path = MountPath::new(root, root_use);
        let mut stack = vec![Frame::new(loadout.ports, path.checkpoint())];
        while let Some(frame) = stack.last_mut() {
            checkpoint(self.cancel)?;
            let Some((index, port)) = frame.ports.next() else {
                path.restore(frame.parent);
                stack.pop();
                continue;
            };
            let parent = path.checkpoint();
            path.position
                .port_ordinals
                .push(loadout_position::ordinal(index)?);
            path.complete &= port.append_name_uses(&mut path.uses)?;
            if let Some((attachment, proof)) = port.alignment().placement_attachment() {
                path.uses.extend_from_slice(&proof.source_uses);
                path.complete &= proof.source_set_complete;
                self.emit(loadout, port, attachment, &path, is_crafter)?;
            }
            path.complete &= !port.state().unresolved;
            if port.children().is_empty() {
                path.restore(parent);
            } else {
                stack.push(Frame::new(port.children(), parent));
            }
        }
        Ok(())
    }

    fn emit(
        &mut self,
        loadout: &LoadoutJoin<'_, impl JoinPort>,
        port: &impl JoinPort,
        attachment: &AttachmentIdentity,
        path: &MountPath,
        is_crafter: &mut impl FnMut(u32) -> Result<bool, CatalogError>,
    ) -> Result<(), CatalogError> {
        let Some(crafter) = attachment.target_record_index().value().copied() else {
            return Ok(());
        };
        if port.state().unresolved || !is_crafter(crafter)? {
            return Ok(());
        }
        self.groups
            .entry(crafter)
            .or_insert_with(|| CrafterHost {
                ship_guid: loadout.entity_guid.sourced().map(|&guid| guid),
                ship_record_index: loadout.source_record_index.clone(),
                crafter: attachment.clone(),
                mount_positions: Vec::new(),
            })
            .mount_positions
            .push(path.sourced(&self.origins.uses));
        Ok(())
    }
}

struct Frame<'loadout, Port> {
    ports: Enumerate<Iter<'loadout, Port>>,
    parent: Checkpoint,
}

impl<'loadout, Port> Frame<'loadout, Port> {
    fn new(ports: &'loadout [Port], parent: Checkpoint) -> Self {
        Self {
            ports: ports.iter().enumerate(),
            parent,
        }
    }
}

#[derive(Clone, Copy)]
struct Checkpoint {
    path_length: usize,
    use_count: usize,
    complete: bool,
}

struct MountPath {
    position: LoadoutPosition,
    uses: Vec<SourceUse>,
    complete: bool,
}

impl MountPath {
    fn new(root: u32, root_use: SourceUse) -> Self {
        Self {
            position: LoadoutPosition::root(root),
            uses: vec![root_use],
            complete: true,
        }
    }

    const fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            path_length: self.position.port_ordinals.len(),
            use_count: self.uses.len(),
            complete: self.complete,
        }
    }

    fn restore(&mut self, checkpoint: Checkpoint) {
        self.position.port_ordinals.truncate(checkpoint.path_length);
        self.uses.truncate(checkpoint.use_count);
        self.complete = checkpoint.complete;
    }

    fn sourced(&self, sources: &SourceUses) -> SourcedValue<LoadoutPosition> {
        sources.value(
            SourceValue::Present(self.position.clone()),
            &self.uses,
            self.complete,
        )
    }
}
