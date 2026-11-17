// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, RecordTree, StructValue, Value};

use super::{ConfigurationKind, EntityConfigurationRows};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_facility::{Facility, OwnerComponents};
use crate::catalog_v3::catalog_item_alignment::{CONTAINER, ItemAlignmentKind, ItemPortContainer};
use crate::catalog_v3::item_alignment_options::AlignmentOptions;
use crate::catalog_v3::item_component_owner::ComponentOwner;
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
    read_scope(tree, projection, admission, options, cancel, Scope::All)
}

pub(crate) fn read_item_ports<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    options: &AlignmentOptions<'_>,
    cancel: Option<&CancelToken>,
) -> Result<(Vec<ItemPortContainer<'source>>, Vec<FieldError>), CatalogError> {
    let (rows, notes) = read_scope(tree, projection, admission, options, cancel, Scope::Ports)?;
    Ok((rows.alignment.into_port_containers(), notes))
}

#[derive(Clone, Copy)]
enum Scope {
    All,
    Ports,
}

impl Scope {
    fn configuration(
        self,
        value: &StructValue<'_>,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Option<ConfigurationKind> {
        match self {
            Self::All => ConfigurationKind::of(value, input),
            Self::Ports if value.type_name() == CONTAINER => Some(ConfigurationKind::Alignment {
                alignment: ItemAlignmentKind::Port,
                occupancy: false,
            }),
            Self::Ports => None,
        }
    }
}

fn read_scope<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    options: &AlignmentOptions<'_>,
    cancel: Option<&CancelToken>,
    requested: Scope,
) -> Result<(EntityConfigurationRows<'source>, Vec<FieldError>), CatalogError> {
    let input = projection.original();
    let mut rows = EntityConfigurationRows::default();
    let mut notes = Vec::new();
    let Some(authored_components @ Value::Array(components)) = tree.field("Components") else {
        return Ok((rows, notes));
    };
    let mut components = components
        .iter()
        .filter_map(|component| {
            let Value::Struct(value) = component else {
                return None;
            };
            requested
                .configuration(value, input)
                .map(|kind| (component, kind))
        })
        .peekable();
    if components.peek().is_none() {
        return Ok((rows, notes));
    }
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let context = input
        .origins()
        .record(tree.record_index())
        .with_cancel(cancel);
    let numbers = RefCell::new(Vec::new());
    for (component, configuration) in components {
        originals.check_cancel()?;
        let component = originals.fact(component)?;
        let owner = ComponentOwner::read(tree, &context, &originals, component)?;
        let source = Observed::new(&originals, input, component, &capture, &numbers);
        match requested {
            Scope::All => rows.push_configuration(
                configuration,
                owner,
                source,
                options,
                projection,
                &mut notes,
            )?,
            Scope::Ports => rows.alignment.push(
                ItemAlignmentKind::Port,
                owner,
                source,
                options,
                projection,
                &mut notes,
            )?,
        }
    }
    retain_cargo_owner(
        &mut rows,
        tree.record_index(),
        &originals,
        authored_components,
    )?;
    rows.numbers = numbers.into_inner();
    Ok((rows, notes))
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
