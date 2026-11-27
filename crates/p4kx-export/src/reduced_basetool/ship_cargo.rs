// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::ReducedError;
use super::cargo_grids::GridFacts;
use super::cargo_placements::InstalledGrid;
use super::cell::{self, Cell};
use crate::catalog_queries::equipment::{CapacityStatus, ShipBaseFacts, ShipCargoMountSource};
use crate::catalog_v3::cargo_source::{self, round_capacity};
use crate::catalog_v3::resource_network_placements::LoadoutPosition;
use crate::guid_text::CatalogGuid;

pub(super) const PATH: &str = "projections.shipBaseFacts";

pub(super) struct ShipCargo {
    pub(super) ship_guid: Cell<CatalogGuid>,
    pub(super) ship_record_index: Cell<u32>,
    canonical: CanonicalCargo,
}

impl From<&ShipBaseFacts> for ShipCargo {
    fn from(facts: &ShipBaseFacts) -> Self {
        let cargo = facts.cargo();
        Self {
            ship_guid: Cell::from(&facts.ship_guid()).map(|guid| CatalogGuid(*guid)),
            ship_record_index: Cell::from(facts.ship_record_index()).map(|value| *value),
            canonical: CanonicalCargo {
                scu: Cell::from(cargo.scu()).map(|value| *value),
                grids_with_issue: Cell::from(cargo.grids_with_issue()).map(|value| *value),
                unresolved_loadout: cargo.has_unresolved_loadout(),
                mounts: cargo.mounts().iter().map(Mount::from).collect(),
            },
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalCargo {
    scu: Cell<f64>,
    grids_with_issue: Cell<u64>,
    unresolved_loadout: bool,
    mounts: Vec<Mount>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Mount {
    position: Cell<LoadoutPosition>,
    capacity_scu: Cell<f64>,
    #[serde(skip)]
    attachment_index: Option<u32>,
    #[serde(skip)]
    attachment_guid: Option<CatalogGuid>,
    #[serde(skip)]
    inventory_index: Option<u32>,
}

impl From<&ShipCargoMountSource> for Mount {
    fn from(mount: &ShipCargoMountSource) -> Self {
        let attachment = mount.attachment();
        Self {
            position: Cell::from(mount.position()).map(Clone::clone),
            capacity_scu: Cell::from(mount.capacity_scu()).map(|value| *value),
            attachment_index: attachment.target_record_index().value().copied(),
            attachment_guid: attachment.target_guid().value().copied(),
            inventory_index: mount.inventory_record_index().value().copied(),
        }
    }
}

impl ShipCargo {
    pub(super) fn row<'source>(
        &'source self,
        installed: &BTreeMap<&[u32], Option<InstalledGrid<'_>>>,
    ) -> Result<impl Serialize + 'source, ReducedError> {
        let record = self
            .ship_record_index
            .value
            .ok_or(ReducedError::IncompleteScope(PATH))?;
        Ok(Row {
            ship_guid: &self.ship_guid,
            ship_record_index: &self.ship_record_index,
            cargo: Cargo {
                canonical: &self.canonical,
                freight: Freight::calculate(record, &self.canonical, installed)?,
            },
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Row<'source> {
    ship_guid: &'source Cell<CatalogGuid>,
    ship_record_index: &'source Cell<u32>,
    cargo: Cargo<'source>,
}

#[derive(Serialize)]
struct Cargo<'source> {
    #[serde(flatten)]
    canonical: &'source CanonicalCargo,
    freight: Freight,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Freight {
    scu: Option<f64>,
    complete: bool,
    contains_encrypted_source: Option<bool>,
}

impl Freight {
    fn calculate(
        record: u32,
        original: &CanonicalCargo,
        installed: &BTreeMap<&[u32], Option<InstalledGrid<'_>>>,
    ) -> Result<Self, ReducedError> {
        let mut complete = !original.unresolved_loadout
            && original.grids_with_issue.value == Some(0)
            && original.scu.value.is_some();
        let mut encrypted = original.scu.contains_encrypted_source;
        let mut total = 0.0;
        let mut positions = BTreeSet::new();
        for mount in &original.mounts {
            let Some(position) = mount.position.value.as_ref() else {
                complete = false;
                continue;
            };
            if position.loadout_record_index() != record
                || !positions.insert(position.port_ordinals())
            {
                return Err(ReducedError::IncompleteScope(PATH));
            }
            let Some(link) = installed
                .get(position.port_ordinals())
                .and_then(Option::as_ref)
            else {
                complete = false;
                continue;
            };
            encrypted = cell::merge_encryption(encrypted, link.placement_encrypted);
            encrypted =
                cell::merge_encryption(encrypted, link.grid.mining_only.contains_encrypted_source);
            if let Some(capacity) = &link.grid.capacity {
                encrypted =
                    cell::merge_encryption(encrypted, capacity.scu.contains_encrypted_source);
            }
            let Some((amount, mining)) = mount.amount(link.grid) else {
                complete = false;
                continue;
            };
            if !mining {
                total += amount;
            }
        }
        let total = round_capacity(total);
        complete &= cargo_source::is_valid_closed_capacity(total);
        Ok(Self {
            scu: complete.then_some(total),
            complete,
            contains_encrypted_source: if complete || encrypted == Some(true) {
                encrypted
            } else {
                None
            },
        })
    }
}

impl Mount {
    fn amount(&self, grid: &GridFacts) -> Option<(f64, bool)> {
        let capacity = grid.capacity.as_ref()?;
        let amount = self.capacity_scu.value?;
        let mining = grid.mining_only.value?;
        (self.attachment_index == Some(grid.key.record)
            && self.attachment_guid == Some(grid.key.guid)
            && self.inventory_index.is_some()
            && self.inventory_index == capacity.inventory_record_index
            && matches!(capacity.status, CapacityStatus::Available)
            && capacity.scu.value.map(f64::to_bits) == Some(amount.to_bits())
            && cargo_source::is_valid_closed_capacity(amount))
        .then_some((amount, mining))
    }
}

#[cfg(test)]
#[path = "freight_tests.rs"]
mod tests;
