// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{
    CancelToken, DataForge, FieldOrigin, Filter, SourceUses, SourceValue, SourceValueState,
    SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use super::Facility;
use crate::catalog_v3::inventory_container_definition::{self, PreparedInventoryContainer};
use crate::catalog_v3::inventory_owner_storage::InventoryOwnerStorage;
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceRecordReference;
use crate::{CatalogError, ExportAccountingError};

#[path = "facility_cargo_operands.rs"]
mod operands;
#[path = "facility_cargo_owner.rs"]
mod owner;

pub(in crate::catalog_v3) use owner::OwnerComponents;
use owner::StorageSelection;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Sourced capacity and the exact authored storage identities.
pub struct CargoGridCapacity {
    storage_component_ordinal: Option<SourcedValue<u32>>,
    container_record_index: Option<SourcedValue<u32>>,
    status: CapacityStatus,
    scu: SourcedValue<f64>,
}

/// Outcome of the original Cargo Grid capacity projection.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[non_exhaustive]
pub enum CapacityStatus {
    /// Capacity was computed from the original inventory definition.
    Available,
    /// The original Component slots cannot establish the storage selection.
    OwnerComponentsUnreadable,
    /// Several original Grid or Storage slots compete.
    AmbiguousComponents,
    /// The owner has no item storage component.
    MissingStorageComponent,
    /// The selected storage has no authored container Reference.
    MissingContainerReference,
    /// The authored Reference cannot select an inventory definition.
    UnresolvedContainerReference,
    /// The nominal GUID names several physical Records.
    AmbiguousContainerReference,
    /// The target definition is outside the producer's active filter.
    ContainerOutsideSelection,
    /// The original selected inventory definition is unavailable.
    ContainerUnavailable,
    /// The selected definition supplies no capacity operand.
    MissingCapacity,
    /// Original operands could not be read.
    UnreadableCapacity,
    /// The authored storage policy or units are unsupported.
    UnsupportedCapacity,
    /// Authored operands cannot represent a valid capacity.
    InvalidCapacity,
}

impl CargoGridCapacity {
    /// Returns the exact storage component ordinal established by the original selection.
    #[must_use]
    pub const fn storage_component_ordinal(&self) -> Option<&SourcedValue<u32>> {
        self.storage_component_ordinal.as_ref()
    }

    /// Returns the physical inventory Record link established by the original selection.
    #[must_use]
    pub const fn container_record_index(&self) -> Option<&SourcedValue<u32>> {
        self.container_record_index.as_ref()
    }

    /// Returns the recorded capacity outcome.
    #[must_use]
    pub const fn status(&self) -> CapacityStatus {
        self.status
    }

    /// Returns the sourced numerical capacity in standard cargo units.
    #[must_use]
    pub const fn scu(&self) -> &SourcedValue<f64> {
        &self.scu
    }
}

pub(in crate::catalog_v3) fn target_for_owner<'rows, 'source: 'rows>(
    owner: &OwnerComponents,
    mut storages: impl Iterator<Item = &'rows InventoryOwnerStorage<'source>>,
    forge: &DataForge<'_>,
) -> Result<Option<u32>, CatalogError> {
    let StorageSelection::Unique(ordinal) = owner.storage else {
        return Ok(None);
    };
    let ordinal = u32::try_from(ordinal).map_err(|_| ExportAccountingError::CounterOverflow)?;
    let storage = storages
        .find(|row| row.owner.key() == (Some(owner.record), Some(ordinal)))
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let reference = storage
        .item_container_params()
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    Ok(reference
        .value()
        .and_then(|reference| container_record(forge, reference, None).ok()))
}

pub(in crate::catalog_v3) fn project<'rows, 'source: 'rows>(
    facilities: &mut [Facility<'source>],
    components: &[OwnerComponents],
    storages: impl Iterator<Item = &'rows InventoryOwnerStorage<'source>>,
    definitions: &[PreparedInventoryContainer<'source>],
    forge: &DataForge<'_>,
    sources: &SourceUses,
    (filter, cancel): (Option<&Filter>, Option<&CancelToken>),
) -> Result<(), CatalogError> {
    if components.is_empty() {
        return Ok(());
    }
    let mut owners = HashMap::new();
    for component in components {
        check_cancel(cancel)?;
        if owners.insert(component.record, component).is_some() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
    }
    let mut storage = HashMap::new();
    for row in storages {
        check_cancel(cancel)?;
        let (Some(record), Some(ordinal)) = row.owner.key() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        if owners.contains_key(&record) && storage.insert((record, ordinal), row).is_some() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
    }
    let mut containers = HashMap::new();
    for definition in definitions {
        check_cancel(cancel)?;
        if containers
            .insert(definition.physical_index, definition)
            .is_some()
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
    }
    let inputs = Inputs {
        forge,
        sources,
        filter,
        storage,
        containers,
    };
    for grid in facilities
        .iter_mut()
        .filter(|facility| facility.cargo_grid())
    {
        check_cancel(cancel)?;
        let index = grid
            .owner()
            .key()
            .0
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let owner = owners
            .get(&index)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let capacity = inputs.capacity(grid, owner)?;
        grid.set_cargo_capacity(capacity)?;
    }
    Ok(())
}

struct Inputs<'rows, 'source, 'forge> {
    forge: &'rows DataForge<'forge>,
    sources: &'rows SourceUses,
    filter: Option<&'rows Filter>,
    storage: HashMap<(u32, u32), &'rows InventoryOwnerStorage<'source>>,
    containers: HashMap<u32, &'rows PreparedInventoryContainer<'source>>,
}

struct Binding<'rows> {
    origins: Vec<&'rows FieldOrigin>,
    storage: Option<&'rows SourcedValue<u32>>,
    container: Option<&'rows SourcedValue<u32>>,
}

impl Inputs<'_, '_, '_> {
    fn capacity(
        &self,
        grid: &Facility<'_>,
        owner: &OwnerComponents,
    ) -> Result<CargoGridCapacity, CatalogError> {
        let binding = Binding::new(grid, owner);
        match owner.storage {
            StorageSelection::Unique(ordinal) => {
                let ordinal =
                    u32::try_from(ordinal).map_err(|_| ExportAccountingError::CounterOverflow)?;
                let storage = self
                    .storage
                    .get(&(owner.record, ordinal))
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
                self.read_storage(storage, binding)
            }
            StorageSelection::Missing => binding.finish(
                self.sources,
                CapacityStatus::MissingStorageComponent,
                SourceValue::Absent,
            ),
            StorageSelection::Ambiguous => binding.finish(
                self.sources,
                CapacityStatus::AmbiguousComponents,
                SourceValue::Invalid,
            ),
            StorageSelection::Unreadable => binding.finish(
                self.sources,
                CapacityStatus::OwnerComponentsUnreadable,
                SourceValue::Unreadable,
            ),
        }
    }

    fn read_storage<'read>(
        &'read self,
        storage: &'read InventoryOwnerStorage<'_>,
        mut binding: Binding<'read>,
    ) -> Result<CargoGridCapacity, CatalogError> {
        let selector = storage.owner.component_ordinal();
        let reference = storage
            .item_container_params()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        binding.storage = Some(selector);
        binding.origins.extend([
            storage.source_type().origin(),
            selector.origin(),
            reference.origin(),
        ]);
        match reference.value() {
            Some(reference) => self.read_target(reference, binding),
            None => binding.finish(
                self.sources,
                match reference.state() {
                    SourceValueState::Absent | SourceValueState::Null => {
                        CapacityStatus::MissingContainerReference
                    }
                    _ => CapacityStatus::UnresolvedContainerReference,
                },
                operands::unavailable(reference.state()),
            ),
        }
    }

    fn read_target<'read>(
        &'read self,
        reference: &'read SourceRecordReference<'_>,
        mut binding: Binding<'read>,
    ) -> Result<CargoGridCapacity, CatalogError> {
        let definition = container_record(self.forge, reference, self.filter).and_then(|index| {
            self.containers
                .get(&index)
                .ok_or(CapacityStatus::ContainerUnavailable)
        });
        match definition {
            Ok(definition) => {
                binding.record(reference, self.filter.is_some());
                let value = operands::calculate(definition, &mut binding.origins);
                let status = match value {
                    SourceValue::Present(_) => CapacityStatus::Available,
                    SourceValue::Absent | SourceValue::Null => CapacityStatus::MissingCapacity,
                    SourceValue::Unsupported => CapacityStatus::UnsupportedCapacity,
                    SourceValue::Invalid => CapacityStatus::InvalidCapacity,
                    _ => CapacityStatus::UnreadableCapacity,
                };
                binding.finish(self.sources, status, value)
            }
            Err(status) => {
                if matches!(
                    status,
                    CapacityStatus::ContainerUnavailable
                        | CapacityStatus::ContainerOutsideSelection
                ) {
                    binding.record(reference, self.filter.is_some());
                }
                let value = if matches!(status, CapacityStatus::AmbiguousContainerReference) {
                    SourceValue::Invalid
                } else {
                    SourceValue::Unreadable
                };
                binding.finish(self.sources, status, value)
            }
        }
    }
}

fn container_record(
    forge: &DataForge<'_>,
    reference: &SourceRecordReference<'_>,
    filter: Option<&Filter>,
) -> Result<u32, CapacityStatus> {
    let guid = reference
        .guid
        .value()
        .ok_or(CapacityStatus::UnresolvedContainerReference)?;
    let mut matches = forge.records_by_guid(guid.0);
    let record = matches
        .next()
        .ok_or(CapacityStatus::UnresolvedContainerReference)?;
    if matches.next().is_some() {
        return Err(CapacityStatus::AmbiguousContainerReference);
    }
    if reference.resolved_index() != Some(record.index())
        || record.type_name() != Ok(inventory_container_definition::SOURCE_TYPE)
    {
        return Err(CapacityStatus::UnresolvedContainerReference);
    }
    let path = reference
        .record_path
        .value()
        .ok_or(CapacityStatus::ContainerUnavailable)?;
    if filter.is_some_and(|filter| !filter.matches(path)) {
        return Err(CapacityStatus::ContainerOutsideSelection);
    }
    Ok(record.index())
}

impl<'rows> Binding<'rows> {
    fn new(grid: &'rows Facility<'_>, owner: &'rows OwnerComponents) -> Self {
        Self {
            origins: vec![
                &owner.origin,
                grid.owner().record_index().origin(),
                grid.owner().component_ordinal().origin(),
                grid.source_type().origin(),
            ],
            storage: None,
            container: None,
        }
    }

    fn record(&mut self, reference: &'rows SourceRecordReference<'_>, filtered: bool) {
        self.container = Some(&reference.target_record_index);
        self.origins.push(reference.target_record_index.origin());
        if filtered || reference.record_path.value().is_none() {
            self.origins.push(reference.record_path.origin());
        }
    }

    fn finish(
        self,
        sources: &SourceUses,
        status: CapacityStatus,
        value: SourceValue<f64>,
    ) -> Result<CargoGridCapacity, CatalogError> {
        let incomplete =
            matches!(value, SourceValue::Unreadable).then(|| sources.origin(&[], false));
        let mut origins: Vec<&FieldOrigin> = self.origins;
        origins.extend(incomplete.iter());
        Ok(CargoGridCapacity {
            storage_component_ordinal: self.storage.cloned(),
            container_record_index: self.container.cloned(),
            status,
            scu: sources.value_with_origins(value, &origins)?,
        })
    }
}

fn check_cancel(cancel: Option<&CancelToken>) -> Result<(), CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    Ok(())
}
