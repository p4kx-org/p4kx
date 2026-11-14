// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::catalog_table::CatalogTable;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceText;

const HACKING_CHIP: &str = "HackingChipParams";
const REMOVABLE_CHIP: &str = "RemovableChipParams";
const MODIFIER: &str = "RemovableChipValue";

#[derive(Clone, Copy)]
pub(super) enum HackingComponent {
    Charge,
    Removable,
}

impl HackingComponent {
    pub(super) fn of(source_type: &str) -> Option<Self> {
        match source_type {
            HACKING_CHIP => Some(Self::Charge),
            REMOVABLE_CHIP => Some(Self::Removable),
            _ => None,
        }
    }
}

/// The original chip component on its physical Item owner.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct HackingChip<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    source_type: SourcedValue<&'source str>,
    configuration: SourcedValue<HackingConfiguration<'source>>,
}

impl<'source> HackingChip<'source> {
    pub(super) fn read(
        kind: HackingComponent,
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let source_type = source.source_type_value()?;
        let configuration = match kind {
            HackingComponent::Charge => HackingConfiguration::HackingChip {
                value: ChargeCount {
                    max_charges: source.int32("maxCharges", notes)?,
                },
            },
            HackingComponent::Removable => HackingConfiguration::RemovableChip {
                value: RemovableValues {
                    values: source.class_array("values", &[MODIFIER], notes, Modifier::read)?,
                },
            },
        };
        let configuration = source_type.clone().map(|_| configuration);
        Ok(Self {
            owner,
            source_type,
            configuration,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
enum HackingConfiguration<'source> {
    HackingChip { value: ChargeCount },
    RemovableChip { value: RemovableValues<'source> },
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ChargeCount {
    max_charges: SourcedValue<i32>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
struct RemovableValues<'source> {
    values: SourcedValue<Vec<SourcedValue<Modifier<'source>>>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
struct Modifier<'source> {
    name: SourcedValue<SourceText<'source>>,
    value: SourcedValue<ResourceFloat32>,
}

impl<'source> Modifier<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            name: source
                .borrowed_string("name", notes)?
                .map(|text| SourceText::read(MODIFIER, "name", text)),
            value: source.single("value", notes)?,
        })
    }
}

pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    mut rows: Vec<HackingChip<'source>>,
) -> CatalogTable<HackingChip<'source>> {
    rows.sort_by_key(|row| row.owner.key());
    CatalogTable::new(forge, &[HACKING_CHIP, REMOVABLE_CHIP], rows)
}
