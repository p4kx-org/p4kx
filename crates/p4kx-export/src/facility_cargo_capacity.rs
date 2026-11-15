// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{
    CancelToken, DataForge, FieldOrigin, Filter, SourceUse, SourceUseRole, SourceUses, SourceValue,
    SourceValueState, SourcedValue, Value,
};
use schemars::JsonSchema;
use serde::Serialize;

use super::{Facility, FacilityKind};
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::catalog_v3::inventory_container_definition::{self, PreparedInventoryContainer};
use crate::catalog_v3::inventory_owner_storage::{self, InventoryOwnerStorage};
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceRecordReference;
use crate::{CatalogError, CatalogOptions, ExportAccountingError};

#[path = "facility_cargo_operands.rs"]
mod operands;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(in crate::catalog_v3) struct CargoGridCapacity {
    storage_component_ordinal: Option<SourcedValue<u32>>,
    container_record_index: Option<SourcedValue<u32>>,
    status: CapacityStatus,
    scu: SourcedValue<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
enum CapacityStatus {
    Available,
    OwnerComponentsUnreadable,
    AmbiguousComponents,
    MissingStorageComponent,
    MissingContainerReference,
    UnresolvedContainerReference,
    AmbiguousContainerReference,
    ContainerOutsideSelection,
    ContainerUnavailable,
    MissingCapacity,
    UnreadableCapacity,
    UnsupportedCapacity,
    InvalidCapacity,
}

pub(in crate::catalog_v3) struct OwnerComponents {
    record: u32,
    storage: StorageSelection,
    origin: FieldOrigin,
}

#[derive(Clone, Copy)]
enum StorageSelection {
    Unique(usize),
    Missing,
    Ambiguous,
    Unreadable,
}

impl OwnerComponents {
    pub(in crate::catalog_v3) fn read(
        record: u32,
        originals: &OriginalFlowFields<'_, '_, '_>,
        components: &Value<'_>,
    ) -> Result<Self, CatalogError> {
        let fact = originals.fact(components)?;
        originals.span(fact)?;
        let Value::Array(slots) = fact.value() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let mut selectors = vec![fact.observe(OriginalValuePart::Value, SourceUseRole::Selector)?];
        let storage = Self::select_storage(originals, slots, &mut selectors)?;
        let origin = originals
            .sources()
            .origin(&selectors, !matches!(storage, StorageSelection::Unreadable));
        Ok(Self {
            record,
            storage,
            origin,
        })
    }

    fn select_storage(
        originals: &OriginalFlowFields<'_, '_, '_>,
        slots: &[Value<'_>],
        selectors: &mut Vec<SourceUse>,
    ) -> Result<StorageSelection, CatalogError> {
        let mut grid = false;
        let mut storage = None;
        let mut unreadable = false;
        for (ordinal, slot) in slots.iter().enumerate() {
            originals.check_cancel()?;
            let fact = originals.fact(slot)?;
            match slot {
                Value::Struct(component) => {
                    selectors.push(
                        fact.observe(OriginalValuePart::ConcreteType, SourceUseRole::Selector)?,
                    );
                    let source_type = component.type_name();
                    if source_type == inventory_owner_storage::ITEM_COMPONENT {
                        if storage.replace(ordinal).is_some() {
                            return Ok(StorageSelection::Ambiguous);
                        }
                    } else if matches!(FacilityKind::of(source_type), Some(FacilityKind::CargoGrid))
                    {
                        if grid {
                            return Ok(StorageSelection::Ambiguous);
                        }
                        grid = true;
                    }
                }
                Value::Null => {
                    selectors
                        .push(fact.observe(OriginalValuePart::Value, SourceUseRole::Selector)?);
                }
                _ => {
                    selectors
                        .push(fact.observe(OriginalValuePart::Value, SourceUseRole::Attempted)?);
                    unreadable = true;
                }
            }
        }
        if unreadable {
            return Ok(StorageSelection::Unreadable);
        }
        Ok(storage.map_or(StorageSelection::Missing, StorageSelection::Unique))
    }
}

pub(in crate::catalog_v3) fn project<'rows, 'source: 'rows>(
    facilities: &mut [Facility<'source>],
    components: &[OwnerComponents],
    storages: impl Iterator<Item = &'rows InventoryOwnerStorage<'source>>,
    definitions: &[PreparedInventoryContainer<'source>],
    forge: &DataForge<'_>,
    sources: &SourceUses,
    options: &CatalogOptions<'_>,
) -> Result<(), CatalogError> {
    if components.is_empty() {
        return Ok(());
    }
    let mut owners = HashMap::new();
    for component in components {
        check_cancel(options.cancel)?;
        if owners.insert(component.record, component).is_some() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
    }
    let mut storage = HashMap::new();
    for row in storages {
        check_cancel(options.cancel)?;
        let (Some(record), Some(ordinal)) = row.owner.key() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        if owners.contains_key(&record) && storage.insert((record, ordinal), row).is_some() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
    }
    let mut containers = HashMap::new();
    for definition in definitions {
        check_cancel(options.cancel)?;
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
        filter: options.filter,
        storage,
        containers,
    };
    for grid in facilities
        .iter_mut()
        .filter(|facility| facility.cargo_grid())
    {
        check_cancel(options.cancel)?;
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

impl<'source> Inputs<'_, 'source, '_> {
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
        match self.target(reference) {
            Target::Definition(definition) => {
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
            Target::MissingDefinition(status) => {
                binding.record(reference, self.filter.is_some());
                binding.finish(self.sources, status, SourceValue::Unreadable)
            }
            Target::Unresolved => binding.finish(
                self.sources,
                CapacityStatus::UnresolvedContainerReference,
                SourceValue::Unreadable,
            ),
            Target::Ambiguous => binding.finish(
                self.sources,
                CapacityStatus::AmbiguousContainerReference,
                SourceValue::Invalid,
            ),
        }
    }

    fn target(&self, reference: &SourceRecordReference<'_>) -> Target<'_, 'source> {
        let Some(guid) = reference.guid.value() else {
            return Target::Unresolved;
        };
        let mut matches = self.forge.records_by_guid(guid.0);
        let Some(record) = matches.next() else {
            return Target::Unresolved;
        };
        if matches.next().is_some() {
            return Target::Ambiguous;
        }
        if reference.resolved_index() != Some(record.index())
            || record.type_name() != Ok(inventory_container_definition::SOURCE_TYPE)
        {
            return Target::Unresolved;
        }
        let Some(&path) = reference.record_path.value() else {
            return Target::MissingDefinition(CapacityStatus::ContainerUnavailable);
        };
        if self.filter.is_some_and(|filter| !filter.matches(path)) {
            return Target::MissingDefinition(CapacityStatus::ContainerOutsideSelection);
        }
        self.containers.get(&record.index()).copied().map_or(
            Target::MissingDefinition(CapacityStatus::ContainerUnavailable),
            Target::Definition,
        )
    }
}

enum Target<'rows, 'source> {
    Definition(&'rows PreparedInventoryContainer<'source>),
    MissingDefinition(CapacityStatus),
    Unresolved,
    Ambiguous,
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
