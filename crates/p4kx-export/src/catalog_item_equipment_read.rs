// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree};

use super::super::ItemComponentRows;
use super::{
    Component, ComponentScope, ItemEquipmentFamily, component_selection_complete, components,
    with_components,
};
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_v3::catalog_durability::{
    Degradation, Distortion, Durability, DurabilityComponent,
};
use crate::catalog_v3::catalog_emp::Emp;
use crate::catalog_v3::catalog_explosives::Explosive;
use crate::catalog_v3::catalog_explosives::catalog_missile_configuration::MissileConfiguration;
use crate::catalog_v3::catalog_jump_drive::JumpDrive;
use crate::catalog_v3::catalog_quantum_drive::QuantumDrive;
use crate::catalog_v3::catalog_quantum_interdiction::QuantumInterdictionGenerator;
use crate::catalog_v3::catalog_shield_generator::ShieldGenerator;
use crate::record_observation::RecordObservation;
use crate::source_value_catalog;
use crate::{CatalogError, ExportAccountingError};

pub(crate) fn read_emp<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<Emp<'source>>>, CatalogError> {
    read(
        tree,
        projection,
        admission,
        cancel,
        ItemEquipmentFamily::Emp,
        |rows| rows.emp,
    )
}

pub(crate) fn read_explosives<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<Explosive<'source>>>, CatalogError> {
    read(
        tree,
        projection,
        admission,
        cancel,
        ItemEquipmentFamily::Explosives,
        |rows| rows.explosives,
    )
}

pub(crate) fn read_missile_configurations<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<MissileConfiguration<'source>>>, CatalogError> {
    read(
        tree,
        projection,
        admission,
        cancel,
        ItemEquipmentFamily::MissileConfigurations,
        |rows| rows.missile_configurations,
    )
}

pub(crate) fn read_health<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<Durability<'source>>>, CatalogError> {
    read(
        tree,
        projection,
        admission,
        cancel,
        ItemEquipmentFamily::Durability(DurabilityComponent::Health),
        |rows| rows.durability.health,
    )
}

pub(crate) fn read_degradation<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<Degradation>>, CatalogError> {
    read(
        tree,
        projection,
        admission,
        cancel,
        ItemEquipmentFamily::Durability(DurabilityComponent::Degradation),
        |rows| rows.durability.degradation,
    )
}

pub(crate) fn read_distortion<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<Distortion>>, CatalogError> {
    read(
        tree,
        projection,
        admission,
        cancel,
        ItemEquipmentFamily::Durability(DurabilityComponent::Distortion),
        |rows| rows.durability.distortion,
    )
}

pub(crate) fn read_shield_generators<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<ShieldGenerator<'source>>>, CatalogError> {
    read(
        tree,
        projection,
        admission,
        cancel,
        ItemEquipmentFamily::ShieldGenerators,
        |rows| rows.shield_generators,
    )
}

pub(crate) fn read_quantum_drives<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<QuantumDrive<'source>>>, CatalogError> {
    read(
        tree,
        projection,
        admission,
        cancel,
        ItemEquipmentFamily::QuantumDrives,
        |rows| rows.quantum_drives,
    )
}

pub(crate) fn read_jump_drives<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<JumpDrive<'source>>>, CatalogError> {
    read(
        tree,
        projection,
        admission,
        cancel,
        ItemEquipmentFamily::JumpDrives,
        |rows| rows.jump_drives,
    )
}

pub(crate) fn read_quantum_interdiction_generators<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<QuantumInterdictionGenerator<'source>>>, CatalogError> {
    read(
        tree,
        projection,
        admission,
        cancel,
        ItemEquipmentFamily::QuantumInterdictionGenerators,
        |rows| rows.quantum_interdiction,
    )
}

fn read<'source, Row>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
    family: ItemEquipmentFamily,
    selected_rows: impl FnOnce(ItemComponentRows<'source>) -> Vec<Row>,
) -> Result<ScopedSourceRead<Vec<Row>>, CatalogError> {
    let selected = ComponentScope::Equipment(family);
    with_components(
        tree,
        projection,
        admission,
        cancel,
        selected,
        |result, originals, _, _| {
            let complete = component_selection_complete(tree)
                && retained_components_complete(tree, originals, family)?;
            Ok(ScopedSourceRead::new(
                selected_rows(result.rows),
                result.notes,
                complete,
            ))
        },
    )
}

fn retained_components_complete(
    tree: &RecordTree<'_>,
    originals: &OriginalFlowFields<'_, '_, '_>,
    family: ItemEquipmentFamily,
) -> Result<bool, CatalogError> {
    for (value, component) in components(tree).iter().filter_map(Component::of) {
        if family.contains(component) {
            let span = originals.span(originals.fact(value)?)?;
            let facts = originals
                .facts()
                .get(span)
                .ok_or(ExportAccountingError::MissingFieldInspection)?;
            if !source_value_catalog::retained_facts_complete(facts) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
