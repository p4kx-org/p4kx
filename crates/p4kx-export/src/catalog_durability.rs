// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{
    CancelToken, DataForge, RecordTree, SourceUseRole, SourceValue, SourcedValue, Value,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::CatalogRecordContext;
use crate::catalog_read::original::{
    OriginalFlowFields, OriginalValueEdge, OriginalValueFact, OriginalValuePart,
};
use crate::catalog_table::CatalogTable;
use crate::guid_text::CatalogGuid;
use crate::record_fields::number_value;
use crate::record_header_field::RecordHeaderField;
use crate::record_observation::RecordObservation;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, ExportAccountingError};

use super::{authored_damage as damage, authored_fields as observed, authored_link as link};

#[path = "durability_degradation.rs"]
mod degradation;
#[path = "durability_distortion.rs"]
mod distortion;
#[path = "durability_event_actions.rs"]
mod event_actions;
#[path = "durability_health_fields.rs"]
mod health_fields;
#[path = "durability_initial_damage.rs"]
mod initial_damage;
#[path = "durability_lifecycle.rs"]
mod lifecycle;
#[path = "durability_resistance.rs"]
mod resistance;
#[path = "durability_wear.rs"]
mod wear;

use degradation::DegradationFields;
use distortion::DistortionFields;
use health_fields::HealthFields;
use observed::Observed;

const HEALTH: &str = "SHealthComponentParams";
const DEGRADATION: &str = "SDegradationParams";
const DISTORTION: &str = "SDistortionParams";

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ComponentOwner {
    owner_guid: SourcedValue<CatalogGuid>,
    owner_record_index: SourcedValue<u32>,
    component_ordinal: SourcedValue<u32>,
}

impl ComponentOwner {
    fn read(
        tree: &RecordTree<'_>,
        context: &CatalogRecordContext<'_>,
        original: &OriginalFlowFields<'_, '_, '_>,
        component: &OriginalValueFact<'_, '_, '_>,
    ) -> Result<Self, CatalogError> {
        let OriginalValueEdge::Element { ordinal } = component.edge() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let ordinal = u32::try_from(ordinal).map_err(|_| ExportAccountingError::CounterOverflow)?;
        Ok(Self {
            owner_guid: context.header_value(
                RecordHeaderField::Guid,
                SourceValue::Present(CatalogGuid(tree.guid())),
            )?,
            owner_record_index: context.header_selector(
                RecordHeaderField::RecordIndex,
                SourceValue::Present(tree.record_index()),
            )?,
            component_ordinal: component
                .bind(
                    OriginalValuePart::Value,
                    SourceValue::Present(ordinal),
                    SourceUseRole::Conversion,
                    true,
                )?
                .into_cell(original.sources()),
        })
    }

    const fn key(&self) -> (Option<u32>, Option<u32>) {
        (
            self.owner_record_index.value().copied(),
            self.component_ordinal.value().copied(),
        )
    }
}

/// One original Health component on a selected item.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
pub(crate) struct Durability<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    #[serde(flatten)]
    fields: HealthFields<'source>,
}

/// One original degradation component on a selected item.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
pub(crate) struct Degradation {
    #[serde(flatten)]
    owner: ComponentOwner,
    #[serde(flatten)]
    fields: DegradationFields,
}

/// One original distortion component on a selected item.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
pub(crate) struct Distortion {
    #[serde(flatten)]
    owner: ComponentOwner,
    #[serde(flatten)]
    fields: DistortionFields,
}

#[derive(Default)]
pub(crate) struct DurabilityRows<'source> {
    health: Vec<Durability<'source>>,
    degradation: Vec<Degradation>,
    distortion: Vec<Distortion>,
    pub(crate) numbers: Vec<CapturedNumber>,
}

impl DurabilityRows<'_> {
    pub(crate) fn extend(&mut self, other: Self) {
        self.health.extend(other.health);
        self.degradation.extend(other.degradation);
        self.distortion.extend(other.distortion);
        self.numbers.extend(other.numbers);
    }
}

#[derive(Default)]
pub(crate) struct DurabilityRead<'source> {
    pub(crate) rows: DurabilityRows<'source>,
    pub(crate) notes: Vec<FieldError>,
    pub(crate) legacy_health: Option<f32>,
}

impl<'source> DurabilityRead<'source> {
    fn push(
        &mut self,
        kind: Component,
        owner: ComponentOwner,
        original: Observed<'_, '_, '_, 'source>,
    ) -> Result<(), CatalogError> {
        match kind {
            Component::Health => {
                let (fields, legacy) = HealthFields::read(original, &mut self.notes)?;
                if self.rows.health.is_empty() {
                    self.legacy_health = legacy;
                }
                self.rows.health.push(Durability { owner, fields });
            }
            Component::Degradation => self.rows.degradation.push(Degradation {
                owner,
                fields: DegradationFields::read(original, &mut self.notes)?,
            }),
            Component::Distortion => self.rows.distortion.push(Distortion {
                owner,
                fields: DistortionFields::read(original, &mut self.notes)?,
            }),
        }
        Ok(())
    }
}

/// Reads each recognized physical component occurrence using its own admitted Record tree.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<DurabilityRead<'source>, CatalogError> {
    let mut result = DurabilityRead::default();
    let Some(Value::Array(components)) = tree.field("Components") else {
        return Ok(result);
    };
    let mut components = components.iter().filter_map(Component::of).peekable();
    if components.peek().is_none() {
        return Ok(result);
    }
    let input = projection.original();
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let context = projection
        .origins()
        .record(tree.record_index())
        .with_cancel(cancel);
    let numbers = RefCell::new(Vec::new());

    for (value, kind) in components {
        let component = originals.fact(value)?;
        let owner = ComponentOwner::read(tree, &context, &originals, component)?;
        let observed = Observed::new(&originals, component, &capture, &numbers);
        result.push(kind, owner, observed)?;
    }
    result.rows.numbers = numbers.into_inner();
    Ok(result)
}

/// Projects the first Health component for variant-only reads without a catalog source scope.
pub(crate) fn legacy_health(tree: &RecordTree<'_>) -> Option<f32> {
    let Value::Array(components) = tree.field("Components")? else {
        return None;
    };
    let source = components
        .iter()
        .find_map(|value| match Component::of(value) {
            Some((Value::Struct(source), Component::Health)) => Some(source),
            _ => None,
        })?;
    source.field("Health").and_then(number_value)
}

#[derive(Clone, Copy)]
enum Component {
    Health,
    Degradation,
    Distortion,
}

impl Component {
    fn of<'read, 'source>(value: &'read Value<'source>) -> Option<(&'read Value<'source>, Self)> {
        let Value::Struct(source) = value else {
            return None;
        };
        let kind = match source.type_name() {
            HEALTH => Self::Health,
            DEGRADATION => Self::Degradation,
            DISTORTION => Self::Distortion,
            _ => return None,
        };
        Some((value, kind))
    }
}

pub(crate) struct DurabilityTables<'source> {
    pub(crate) health: CatalogTable<Durability<'source>>,
    pub(crate) degradation: CatalogTable<Degradation>,
    pub(crate) distortion: CatalogTable<Distortion>,
}

/// Orders each family by physical Record and original component slot.
pub(crate) fn tables<'source>(
    forge: &DataForge<'_>,
    mut rows: DurabilityRows<'source>,
) -> DurabilityTables<'source> {
    rows.health.sort_by_key(|row| row.owner.key());
    rows.degradation.sort_by_key(|row| row.owner.key());
    rows.distortion.sort_by_key(|row| row.owner.key());
    DurabilityTables {
        health: CatalogTable::new(forge, &[HEALTH], rows.health),
        degradation: CatalogTable::new(forge, &[DEGRADATION], rows.degradation),
        distortion: CatalogTable::new(forge, &[DISTORTION], rows.distortion),
    }
}
