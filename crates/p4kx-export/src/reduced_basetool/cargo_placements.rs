// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use serde::Serialize;

use super::ReducedError;
use super::cargo_grids::{CargoGrids, GridFacts};
use super::cell::Cell;
use super::component::{Component, ComponentKey};
use crate::catalog_queries::equipment::{FacilityConfigurationKind, FacilityInstance};
use crate::catalog_rows::staged::StagedCatalogRows;
use crate::catalog_v3::resource_network_placements::LoadoutPosition;
use crate::contract_loadout::Loadout;
use crate::guid_text::CatalogGuid;

pub(super) const PATH: &str = "placements.facilityInstances";

pub(super) struct InstalledGrid<'source> {
    pub(super) grid: &'source GridFacts,
    pub(super) placement_encrypted: Option<bool>,
}

type MountedGrids<'source> = BTreeMap<&'source [u32], Option<InstalledGrid<'source>>>;

pub(super) fn stage<'source>(
    loadout: &Loadout,
    placements: &'source [FacilityInstance],
    grids: &'source CargoGrids,
    target: &mut StagedCatalogRows<'_>,
) -> Result<MountedGrids<'source>, ReducedError> {
    let mut installed = BTreeMap::new();
    for placement in placements {
        let vehicle = placement.ship_guid();
        let record = placement.ship_record_index();
        let position = placement
            .loadout_position()
            .ok_or(ReducedError::IncompleteScope(PATH))?;
        let route = position
            .value()
            .ok_or(ReducedError::IncompleteScope(PATH))?;
        if record.value() != Some(&loadout.record_index())
            || vehicle.value().map(|guid| guid.get()) != Some(loadout.entity_guid())
            || route.loadout_record_index() != loadout.record_index()
        {
            return Err(ReducedError::IncompleteScope(PATH));
        }
        let (configurations, grid) = links(placement, grids)?;
        let proof = position.origin().contains_encrypted_source();
        if installed
            .insert(
                route.port_ordinals(),
                grid.map(|grid| InstalledGrid {
                    grid,
                    placement_encrypted: proof,
                }),
            )
            .is_some()
        {
            return Err(ReducedError::IncompleteScope(PATH));
        }
        target.append(&Placement {
            ship_guid: vehicle.into(),
            ship_record_index: record.into(),
            source: PlacementSource::Loadout {
                position: position.into(),
            },
            configurations,
        })?;
    }
    Ok(installed)
}

fn links<'source>(
    placement: &'source FacilityInstance,
    grids: &'source CargoGrids,
) -> Result<(Vec<GridLink<'source>>, Option<&'source GridFacts>), ReducedError> {
    let mut links = Vec::new();
    let mut selected = None;
    for (kind, owner) in placement.configurations() {
        if kind != FacilityConfigurationKind::Facility {
            return Err(ReducedError::IncompleteScope(PATH));
        }
        let key = ComponentKey::read(owner, PATH)?;
        let grid = grids.get(key).ok_or(ReducedError::IncompleteScope(PATH))?;
        let physical_owner = placement.attached_item().map_or_else(
            || {
                placement
                    .ship_record_index()
                    .value()
                    .copied()
                    .zip(placement.ship_guid().value().copied())
            },
            |attachment| {
                attachment
                    .target_record_index()
                    .value()
                    .copied()
                    .zip(attachment.target_guid().value().copied())
            },
        );
        if physical_owner != Some((key.record, key.guid)) {
            return Err(ReducedError::IncompleteScope(PATH));
        }
        selected = Some(grid);
        links.push(GridLink::Facility(Component::from(owner)));
    }
    if links.len() != 1 {
        selected = None;
    }
    Ok((links, selected))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Placement<'source> {
    ship_guid: Cell<&'source CatalogGuid>,
    ship_record_index: Cell<&'source u32>,
    source: PlacementSource<'source>,
    configurations: Vec<GridLink<'source>>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum PlacementSource<'source> {
    Loadout {
        position: Cell<&'source LoadoutPosition>,
    },
}

#[derive(Serialize)]
#[serde(tag = "kind", content = "owner", rename_all = "camelCase")]
enum GridLink<'source> {
    Facility(Component<'source>),
}
