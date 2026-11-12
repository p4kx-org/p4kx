// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, DataForge, RecordTree, StructValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_fuel_configurations::{
    FuelConfigurationKind, FuelConfigurationRows, FuelConfigurationTables,
};
use super::catalog_item_alignment::{ItemAlignmentKind, ItemAlignmentRows, ItemAlignmentTables};
use super::item_alignment_options::AlignmentOptions;
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_read::original::OriginalFlowFields;
use crate::record_observation::RecordObservation;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::template_field_capture::TemplateFieldCapture;

#[derive(Default)]
pub(crate) struct EntityConfigurationRows<'source> {
    pub(crate) alignment: ItemAlignmentRows<'source>,
    fuel: FuelConfigurationRows<'source>,
    pub(crate) numbers: Vec<CapturedNumber>,
}

impl<'source> EntityConfigurationRows<'source> {
    pub(crate) fn extend(&mut self, other: Self) {
        self.alignment.extend(other.alignment);
        self.fuel.extend(other.fuel);
        self.numbers.extend(other.numbers);
    }

    pub(crate) fn into_tables(self, forge: &DataForge<'_>) -> EntityConfigurationTables<'source> {
        EntityConfigurationTables {
            alignment: self.alignment.into_tables(forge),
            fuel: self.fuel.into_tables(forge),
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
pub(crate) struct EntityConfigurationTables<'source> {
    #[serde(flatten)]
    alignment: ItemAlignmentTables<'source>,
    #[serde(flatten)]
    fuel: FuelConfigurationTables<'source>,
}

enum ConfigurationKind {
    Alignment(ItemAlignmentKind),
    Fuel(FuelConfigurationKind),
}

impl ConfigurationKind {
    fn of(value: &StructValue<'_>, input: &OriginalCatalogInput<'_, '_>) -> Option<Self> {
        ItemAlignmentKind::of(value, input)
            .map(Self::Alignment)
            .or_else(|| FuelConfigurationKind::of(value.type_name()).map(Self::Fuel))
    }
}

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    options: &AlignmentOptions<'_>,
    cancel: Option<&CancelToken>,
) -> Result<(EntityConfigurationRows<'source>, Vec<FieldError>), CatalogError> {
    let mut rows = EntityConfigurationRows::default();
    let mut notes = Vec::new();
    let Some(Value::Array(components)) = tree.field("Components") else {
        return Ok((rows, notes));
    };
    let mut components = components
        .iter()
        .filter_map(|component| {
            let Value::Struct(value) = component else {
                return None;
            };
            ConfigurationKind::of(value, input).map(|kind| (component, kind))
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
        match configuration {
            ConfigurationKind::Alignment(alignment) => {
                rows.alignment
                    .push(alignment, owner, source, options, &mut notes)?;
            }
            ConfigurationKind::Fuel(fuel) => rows.fuel.push(fuel, owner, source, &mut notes)?,
        }
    }
    rows.numbers = numbers.into_inner();
    Ok((rows, notes))
}
