// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::ops::Range;

use p4kx_extraction::{CancelToken, RecordTree, Value};

use super::{
    ConfigurationKind, EntityConfigurationRows, FuelConfigurationKind, FuelIntake, FuelTank,
    ResourceComponent, ResourceConnection, ResourceContainer, VehicleArmor,
};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_facility::{Facility, OwnerComponents};
use crate::catalog_v3::catalog_item_alignment::{ItemAttachmentComponent, ItemPortContainer};
use crate::catalog_v3::inventory_owner_bounds::InventoryOwnerBounds;
use crate::catalog_v3::item_alignment_options::AlignmentOptions;
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::export_diagnostic::ExportDiagnosticSeverity;
use crate::export_field_inspection::ConfigurationInspection;
use crate::export_field_walk;
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::template_field_capture::TemplateFieldCapture;

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    options: &AlignmentOptions<'_>,
    cancel: Option<&CancelToken>,
) -> Result<(EntityConfigurationRows<'source>, Vec<FieldError>), CatalogError> {
    let read = read_scope(tree, projection, admission, options, cancel, Scope::All)?;
    Ok((read.value, read.notes))
}

pub(crate) fn read_item_ports<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    options: &AlignmentOptions<'_>,
    cancel: Option<&CancelToken>,
) -> Result<(Vec<ItemPortContainer<'source>>, Vec<FieldError>), CatalogError> {
    let read = read_item_ports_scoped(tree, projection, admission, options, cancel)?;
    Ok((read.value, read.notes))
}

pub(crate) fn read_item_ports_scoped<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    options: &AlignmentOptions<'_>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<ItemPortContainer<'source>>>, CatalogError> {
    let read = read_scope(tree, projection, admission, options, cancel, Scope::Ports)?;
    Ok(read.map(|rows| rows.alignment.into_port_containers()))
}

#[path = "catalog_configuration_scope.rs"]
mod scope;
use self::scope::{ConfigurationOwner, Scope};

pub(crate) fn read_attachment_constraints<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    options: &AlignmentOptions<'_>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<ItemAttachmentComponent<'source>>>, CatalogError> {
    let read = read_scope(
        tree,
        projection,
        admission,
        options,
        cancel,
        Scope::Attachments,
    )?;
    Ok(read.map(|rows| rows.alignment.into_attachment_components()))
}

pub(crate) fn read_item_bounds<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    options: &AlignmentOptions<'_>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<InventoryOwnerBounds<'source>>>, CatalogError> {
    let read = read_scope(tree, projection, admission, options, cancel, Scope::Bounds)?;
    Ok(read.map(|rows| rows.inventory.into_bounds()))
}

pub(crate) fn read_cargo_configurations<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    options: &AlignmentOptions<'_>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<EntityConfigurationRows<'source>>, CatalogError> {
    read_scope(
        tree,
        projection,
        admission,
        options,
        cancel,
        Scope::CargoGrids,
    )
}

pub(crate) fn read_vehicle_armor<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    options: &AlignmentOptions<'_>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<VehicleArmor<'source>>>, CatalogError> {
    let read = read_scope(
        tree,
        projection,
        admission,
        options,
        cancel,
        Scope::VehicleArmor,
    )?;
    Ok(read.map(|rows| rows.vehicle_armor))
}

pub(crate) fn read_resource_connections<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    options: &AlignmentOptions<'_>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<ResourceConnection<'source>>>, CatalogError> {
    let read = read_scope(
        tree,
        projection,
        admission,
        options,
        cancel,
        Scope::ResourceConnections,
    )?;
    Ok(read.map(|rows| rows.connections))
}

pub(crate) fn read_resource_components<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    options: &AlignmentOptions<'_>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<ResourceComponent<'source>>>, CatalogError> {
    let read = read_scope(
        tree,
        projection,
        admission,
        options,
        cancel,
        Scope::ResourceComponents,
    )?;
    Ok(read.map(|rows| rows.resources))
}

pub(crate) fn read_resource_containers<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    options: &AlignmentOptions<'_>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<ResourceContainer<'source>>>, CatalogError> {
    let read = read_scope(
        tree,
        projection,
        admission,
        options,
        cancel,
        Scope::Fuel(FuelConfigurationKind::Container),
    )?;
    Ok(read.map(|rows| rows.fuel.containers))
}

pub(crate) fn read_fuel_intakes<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    options: &AlignmentOptions<'_>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<FuelIntake<'source>>>, CatalogError> {
    let read = read_scope(
        tree,
        projection,
        admission,
        options,
        cancel,
        Scope::Fuel(FuelConfigurationKind::Intake),
    )?;
    Ok(read.map(|rows| rows.fuel.intakes))
}

pub(crate) fn read_fuel_tanks<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    options: &AlignmentOptions<'_>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<FuelTank<'source>>>, CatalogError> {
    let read = read_scope(
        tree,
        projection,
        admission,
        options,
        cancel,
        Scope::Fuel(FuelConfigurationKind::Tank),
    )?;
    Ok(read.map(|rows| rows.fuel.tanks))
}

fn read_scope<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    options: &AlignmentOptions<'_>,
    cancel: Option<&CancelToken>,
    requested: Scope,
) -> Result<ScopedSourceRead<EntityConfigurationRows<'source>>, CatalogError> {
    let input = projection.original();
    let scope = input.record_scope(admission.record(), tree)?;
    let mut selection_complete = requested.selection_complete(tree, input, cancel)?;
    let mut rows = EntityConfigurationRows::default();
    let mut notes = Vec::new();
    let Some((authored_components, components)) = selected_components(tree, input, requested)
    else {
        return Ok(ScopedSourceRead::new(rows, notes, selection_complete));
    };
    let mut components = components.peekable();
    if components.peek().is_none() {
        return Ok(ScopedSourceRead::new(rows, notes, selection_complete));
    }
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let context = input
        .origins()
        .record(tree.record_index())
        .with_cancel(cancel);
    let numbers = RefCell::new(Vec::new());
    let inspection = requested.inspection();
    let mut spans = Vec::new();
    for (component, configuration) in components {
        originals.check_cancel()?;
        let component = originals.fact(component)?;
        let owner = ComponentOwner::read(tree, &context, &originals, component)?;
        let source = Observed::new(&originals, input, component, &capture, &numbers);
        let selected = ConfigurationOwner {
            configuration,
            owner,
        };
        requested.project(&mut rows, selected, source, options, projection, &mut notes)?;
        if inspection.is_some() {
            spans.push(originals.span(component)?);
        }
    }
    if let Some(inspection) = inspection {
        selection_complete &=
            inspected_scope_complete(input, tree, admission, &spans, inspection, cancel)?;
    }
    retain_cargo_owner(
        &mut rows,
        tree.record_index(),
        &originals,
        authored_components,
    )?;
    rows.numbers = numbers.into_inner();
    Ok(ScopedSourceRead::new(rows, notes, selection_complete))
}

fn inspected_scope_complete<'source>(
    input: &OriginalCatalogInput<'_, 'source>,
    tree: &RecordTree<'source>,
    admission: &RecordObservation<'_, 'source>,
    spans: &[Range<usize>],
    inspection: ConfigurationInspection,
    cancel: Option<&CancelToken>,
) -> Result<bool, CatalogError> {
    let evidence = export_field_walk::inspect_configuration(
        input, tree, admission, spans, inspection, cancel,
    )?;
    Ok(evidence.field_inspections.iter().all(|row| row.complete)
        && evidence
            .field_diagnostics
            .iter()
            .all(|row| row.severity == ExportDiagnosticSeverity::Information))
}

fn retain_cargo_owner(
    rows: &mut EntityConfigurationRows<'_>,
    index: u32,
    originals: &OriginalFlowFields<'_, '_, '_>,
    components: &Value<'_>,
) -> Result<(), CatalogError> {
    if rows.facilities.iter().any(Facility::cargo_grid) {
        rows.cargo_owners
            .push(OwnerComponents::read(index, originals, components)?);
    }
    Ok(())
}

fn selected_components<'scope, 'source>(
    tree: &'scope RecordTree<'source>,
    input: &'scope OriginalCatalogInput<'_, 'source>,
    requested: Scope,
) -> Option<(
    &'scope Value<'source>,
    impl Iterator<Item = (&'scope Value<'source>, ConfigurationKind)> + 'scope,
)> {
    let authored @ Value::Array(components) = tree.field("Components")? else {
        return None;
    };
    if !requested.includes_owner(components) {
        return None;
    }
    let selected = components.iter().filter_map(move |component| {
        let Value::Struct(value) = component else {
            return None;
        };
        requested
            .configuration(value, input)
            .map(|kind| (component, kind))
    });
    Some((authored, selected))
}
