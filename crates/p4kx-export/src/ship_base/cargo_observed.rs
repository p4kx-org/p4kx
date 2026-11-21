// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{EntityNode, LoadoutEntry};
use p4kx_extraction::{Record, RecordTree, SourceUseRole, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::cargo_capacity_origin::{self, CapacitySource};
use super::cargo_container::MeasuredCapacity;
use crate::catalog_v3::resource_network_placements::LoadoutPosition;
use crate::guid_text::CatalogGuid;
use crate::loadout::alignment::AttachmentIdentity;
use crate::loadout::ledger::Capture;
use crate::record_header_field::RecordHeaderField;
use crate::schema_fields::every_field_required;
use crate::ship_base::{Cargo, CargoGrid};
use crate::{CatalogError, ExportAccountingError};

/// Cargo values and the actually reached mounts in the canonical Ship calculation.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ShipCargoSource {
    scu: SourcedValue<f64>,
    grids_with_issue: SourcedValue<u64>,
    unresolved_loadout: bool,
    mounts: Vec<ShipCargoMountSource>,
}

impl ShipCargoSource {
    /// Borrows the known SCU total and its observed positive contributors.
    #[must_use]
    pub const fn scu(&self) -> &SourcedValue<f64> {
        &self.scu
    }

    /// Borrows the number of reached grids whose capacity could not be calculated.
    #[must_use]
    pub const fn grids_with_issue(&self) -> &SourcedValue<u64> {
        &self.grids_with_issue
    }

    /// Returns whether a resolver reported an unread or undecided Loadout branch.
    #[must_use]
    pub const fn has_unresolved_loadout(&self) -> bool {
        self.unresolved_loadout
    }

    /// Borrows the reached mounts in default Loadout traversal order.
    #[must_use]
    pub fn mounts(&self) -> &[ShipCargoMountSource] {
        &self.mounts
    }
}

/// A physical contributing attachment and its selected inventory capacity operands.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ShipCargoMountSource {
    #[serde(skip)]
    #[schemars(skip)]
    port_path: String,
    position: SourcedValue<LoadoutPosition>,
    attachment: AttachmentIdentity,
    storage: SourcedValue<CatalogGuid>,
    inventory_record_index: SourcedValue<u32>,
    capacity_scu: SourcedValue<f64>,
}

impl ShipCargoMountSource {
    /// Borrows the resolved Loadout port path for this occurrence.
    #[must_use]
    pub fn port_path(&self) -> &str {
        &self.port_path
    }

    /// Borrows the resolved physical Loadout and its nested port position.
    #[must_use]
    pub const fn position(&self) -> &SourcedValue<LoadoutPosition> {
        &self.position
    }

    /// Borrows the selected physical attachment and its original choice.
    #[must_use]
    pub const fn attachment(&self) -> &AttachmentIdentity {
        &self.attachment
    }

    /// Borrows the authored `InventoryContainer` reference selected on the attachment.
    #[must_use]
    pub const fn storage(&self) -> &SourcedValue<CatalogGuid> {
        &self.storage
    }

    /// Borrows the first physical inventory Record selected by the canonical calculation.
    #[must_use]
    pub const fn inventory_record_index(&self) -> &SourcedValue<u32> {
        &self.inventory_record_index
    }

    /// Borrows the grid capacity and its exact capacity operands.
    #[must_use]
    pub const fn capacity_scu(&self) -> &SourcedValue<f64> {
        &self.capacity_scu
    }
}

pub(crate) struct Observation<'read, 'source> {
    capture: &'read Capture<'source>,
    mounts: Vec<ShipCargoMountSource>,
    error: Option<CatalogError>,
    root: Option<SourcedValue<u32>>,
    unresolved_loadout: bool,
}

impl<'read, 'source> Observation<'read, 'source> {
    pub(crate) const fn new(capture: &'read Capture<'source>) -> Self {
        Self {
            capture,
            mounts: Vec::new(),
            error: None,
            root: None,
            unresolved_loadout: false,
        }
    }

    pub(crate) fn loadout(
        &mut self,
        record: &Record<'_>,
        root: &EntityNode,
        entries: &[LoadoutEntry<'_>],
    ) {
        match self.root_index(record, root) {
            Ok(index) => self.root = Some(index),
            Err(error) => self.error = Some(error),
        }
        let unresolved = |node: &EntityNode| !node.issues().is_empty() || !node.notes().is_empty();
        self.unresolved_loadout = unresolved(root)
            || entries
                .iter()
                .any(|entry| entry.entity().is_none_or(unresolved));
    }

    fn root_index(
        &self,
        record: &Record<'_>,
        root: &EntityNode,
    ) -> Result<SourcedValue<u32>, CatalogError> {
        if root.record_index() != Some(record.index()) || root.guid() != record.guid() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let (input, _) = self.capture.cargo_context()?;
        input.record_context(record)?.header_selector(
            RecordHeaderField::RecordIndex,
            SourceValue::Present(record.index()),
        )
    }

    pub(super) const fn capture(&self) -> &'read Capture<'source> {
        self.capture
    }

    pub(super) fn mount(
        &mut self,
        entry: &LoadoutEntry<'_>,
        node: &EntityNode,
        grid: &CargoGrid,
        capacity: Option<&CapacitySource>,
    ) {
        if self.error.is_some() {
            return;
        }
        let mount = self
            .root
            .as_ref()
            .ok_or(ExportAccountingError::UnobservedFailure)
            .map_err(CatalogError::from)
            .and_then(|root| capture_mount(self.capture, root, entry, node, grid, capacity));
        match mount {
            Ok(mount) => self.mounts.push(mount),
            Err(error) => self.error = Some(error),
        }
    }

    pub(crate) fn finish(self, cargo: &Cargo) -> Result<ShipCargoSource, CatalogError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if cargo.grids.len() != self.mounts.len() {
            return Err(ExportAccountingError::UnobservedFailure.into());
        }
        let (input, _) = self.capture.cargo_context()?;
        let uses = &input.origins().uses;
        // Reached mounts do not establish why other resolver branches were excluded.
        let unproven_membership = uses.origin(&[], false);
        let mut origins = vec![&unproven_membership];
        for mount in &self.mounts {
            origins.extend([
                mount.attachment.target_record_index().origin(),
                mount.storage.origin(),
                mount.capacity_scu.origin(),
            ]);
        }
        let scu = uses.value_with_origins(
            cargo
                .scu
                .map_or(SourceValue::Unreadable, SourceValue::Present),
            &origins,
        )?;
        let issue_count = u64::try_from(cargo.grids_with_issue)
            .map_err(|_| ExportAccountingError::CounterOverflow)?;
        let grids_with_issue =
            uses.value_with_origins(SourceValue::Present(issue_count), &origins)?;
        Ok(ShipCargoSource {
            scu,
            grids_with_issue,
            mounts: self.mounts,
            unresolved_loadout: self.unresolved_loadout,
        })
    }
}

fn capture_mount(
    capture: &Capture<'_>,
    root: &SourcedValue<u32>,
    entry: &LoadoutEntry<'_>,
    node: &EntityNode,
    grid: &CargoGrid,
    capacity: Option<&CapacitySource>,
) -> Result<ShipCargoMountSource, CatalogError> {
    let record_index = node
        .record_index()
        .ok_or(ExportAccountingError::UnobservedFailure)?;
    let (attachment, _) =
        capture.attachment_identity(node.guid(), record_index, node.attachment_target())?;
    let storage = capture.cargo_storage(node, record_index)?;
    let (input, _) = capture.cargo_context()?;
    let uses = &input.origins().uses;
    let root_index = root
        .value()
        .copied()
        .ok_or(ExportAccountingError::UnobservedFailure)?;
    let ordinals = entry
        .port_ordinals()
        .ok_or(ExportAccountingError::UnobservedFailure)?;
    let position = LoadoutPosition::from_port_ordinals(root_index, ordinals)?;
    let incomplete_ancestors = uses.origin(&[], false);
    let position = uses.value_with_origins(
        SourceValue::Present(position),
        &[
            root.origin(),
            attachment.target_record_index().origin(),
            &incomplete_ancestors,
        ],
    )?;
    let state = grid
        .scu
        .map_or(SourceValue::Unreadable, SourceValue::Present);
    let capacity_scu = if let Some(capacity) = capacity {
        uses.value_with_origins(state, &[&capacity.origin])?
    } else {
        if grid.scu.is_some() {
            return Err(ExportAccountingError::UnobservedFailure.into());
        }
        uses.value(state, &[], false)
    };
    Ok(ShipCargoMountSource {
        port_path: entry.path().to_owned(),
        position,
        attachment,
        storage,
        inventory_record_index: capacity.map_or_else(
            || uses.value(SourceValue::Absent, &[], false),
            |source| source.record_index.clone(),
        ),
        capacity_scu,
    })
}

pub(super) fn capacity<'source>(
    capture: &Capture<'_>,
    record: &Record<'source>,
    tree: &RecordTree<'source>,
) -> Option<(MeasuredCapacity<'source>, CapacitySource)> {
    let result = capture
        .cargo_context()
        .and_then(|(input, cancel)| cargo_capacity_origin::read(input, record, tree, cancel));
    match result {
        Ok(source) => Some(source),
        Err(error) => {
            capture.record_error(error);
            None
        }
    }
}

pub(super) fn unreadable(capture: &Capture<'_>, record: &Record<'_>) -> Option<CapacitySource> {
    let result = (|| {
        let (input, _) = capture.cargo_context()?;
        let context = input.record_context(record)?;
        let chosen =
            context.observe_header(RecordHeaderField::RecordIndex, SourceUseRole::Selector)?;
        Ok(CapacitySource {
            record_index: context.header_selector(
                RecordHeaderField::RecordIndex,
                SourceValue::Present(record.index()),
            )?,
            origin: input.origins().uses.origin(&[chosen], false),
        })
    })();
    match result {
        Ok(source) => Some(source),
        Err(error) => {
            capture.record_error(error);
            None
        }
    }
}
