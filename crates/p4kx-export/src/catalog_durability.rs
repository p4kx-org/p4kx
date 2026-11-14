// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, RecordTree, Value};
use schemars::JsonSchema;
use serde::Serialize;

use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::catalog_table::CatalogTable;
use crate::record_fields::number_value;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

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
}

impl<'source> DurabilityRows<'source> {
    pub(super) fn push(
        &mut self,
        kind: DurabilityComponent,
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Option<f32>, CatalogError> {
        match kind {
            DurabilityComponent::Health => {
                let (fields, legacy) = HealthFields::read(source, notes)?;
                let first = self.health.is_empty();
                self.health.push(Durability { owner, fields });
                Ok(if first { legacy } else { None })
            }
            DurabilityComponent::Degradation => {
                self.degradation.push(Degradation {
                    owner,
                    fields: DegradationFields::read(source, notes)?,
                });
                Ok(None)
            }
            DurabilityComponent::Distortion => {
                self.distortion.push(Distortion {
                    owner,
                    fields: DistortionFields::read(source, notes)?,
                });
                Ok(None)
            }
        }
    }

    pub(super) fn extend(&mut self, other: Self) {
        self.health.extend(other.health);
        self.degradation.extend(other.degradation);
        self.distortion.extend(other.distortion);
    }
}

/// Projects the first Health component for variant-only reads without a catalog source scope.
pub(crate) fn legacy_health(tree: &RecordTree<'_>) -> Option<f32> {
    let Value::Array(components) = tree.field("Components")? else {
        return None;
    };
    let source = components.iter().find_map(|value| match value {
        Value::Struct(source) if source.type_name() == HEALTH => Some(source),
        _ => None,
    })?;
    source.field("Health").and_then(number_value)
}

#[derive(Clone, Copy)]
pub(super) enum DurabilityComponent {
    Health,
    Degradation,
    Distortion,
}

impl DurabilityComponent {
    pub(super) fn of(source_type: &str) -> Option<Self> {
        match source_type {
            HEALTH => Some(Self::Health),
            DEGRADATION => Some(Self::Degradation),
            DISTORTION => Some(Self::Distortion),
            _ => None,
        }
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
