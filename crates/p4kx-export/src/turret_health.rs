// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::{CancelToken, DataForge, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::ordered_curve::OrderedCurve;
use super::{ComponentKind, PhysicalComponents};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_entity_component_read::{self, ComponentSelection};
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::catalog_v3::referenced_definition_read::{self, Selection};
use crate::export_field_inspection::ConfigurationInspection;
use crate::export_ledger::ExportLedger;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

pub(crate) const HEALTH_MODIFIER: &str = "STurretHealthModifierDef";
const TURRET: ComponentKind = ComponentKind {
    source_type: "SCItemTurretParams",
    inspection: ConfigurationInspection::TurretHealth,
};

/// The authored health modifier Reference of one physical turret component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct TurretHealth<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    health_modifier_record: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> TurretHealth<'source> {
    fn read(
        owner: ComponentOwner,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let health_modifier_record =
            fields.record_link_to("healthModifierRecord", HEALTH_MODIFIER, notes)?;
        let original = UnitSource::read(
            fields,
            [("healthModifierRecord", health_modifier_record.state())],
        )?;
        Ok(Self {
            owner,
            health_modifier_record,
            original,
        })
    }

    fn modifier_target(&self) -> Option<(u32, &'source str)> {
        let reference = self.health_modifier_record.value()?;
        Some((reference.resolved_index()?, *reference.target_type.value()?))
    }
}

/// One original health modifier Record and its damage movement curve.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct TurretHealthModifier<'source> {
    source: CanonicalRecordHeader<'source>,
    damage_movement_modifier: SourcedValue<OrderedCurve<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> TurretHealthModifier<'source> {
    /// Borrows the original damage movement curve.
    #[must_use]
    pub const fn damage_movement_modifier(&self) -> &SourcedValue<OrderedCurve<'source>> {
        &self.damage_movement_modifier
    }

    /// Borrows remaining original fields.
    #[must_use]
    pub const fn additional_fields(&self) -> &SourceArenaOutput<'source> {
        &self.additional_fields
    }

    /// Borrows the physical definition header.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }

    pub(in crate::catalog_v3) fn read(
        _source_type: &'source str,
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let damage_movement_modifier = fields.declared_class_to(
            "damageMovementModifier",
            "BezierCurve",
            notes,
            OrderedCurve::read_bezier,
        )?;
        let additional_fields = fields.authored_members_except([(
            "damageMovementModifier",
            damage_movement_modifier.state(),
        )])?;
        Ok(Self {
            source,
            damage_movement_modifier,
            additional_fields,
        })
    }
}

pub(super) struct TurretTables<'source> {
    pub(super) health: CatalogTable<TurretHealth<'source>>,
    pub(super) modifiers: CatalogTable<TurretHealthModifier<'source>>,
}

impl<'source> TurretTables<'source> {
    pub(super) fn read(
        components: &mut PhysicalComponents<'_, '_, 'source>,
    ) -> Result<Self, CatalogError> {
        let health = components.table(TURRET, TurretHealth::read)?;
        let (linked, source_types) = modifier_sources(&health.rows);
        let definitions = referenced_definition_read::read(
            components.forge,
            components.projection.original(),
            &Selection {
                source_types: &source_types,
                linked: &linked,
                filter: components.options.filter,
                inspection: ConfigurationInspection::TurretHealthModifier,
            },
            components.options.cancel,
            TurretHealthModifier::read,
        )?;
        let modifiers = CatalogTable::new(
            components.forge,
            &source_types,
            components.keep_definitions(definitions),
        );
        Ok(Self { health, modifiers })
    }
}

fn modifier_sources<'source>(rows: &[TurretHealth<'source>]) -> (BTreeSet<u32>, Vec<&'source str>) {
    let (linked, mut types): (BTreeSet<_>, BTreeSet<_>) = rows
        .iter()
        .filter_map(TurretHealth::modifier_target)
        .unzip();
    types.insert(HEALTH_MODIFIER);
    (linked, types.into_iter().collect())
}

pub(crate) fn observe_modifier_types<'source>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(Vec<&'source str>, ExportLedger), CatalogError> {
    let read = catalog_entity_component_read::read(
        forge,
        input,
        &ComponentSelection::discovery(),
        cancel,
        &TURRET,
        TurretHealth::read,
    )?;
    let (_, types) = modifier_sources(&read.rows);
    Ok((types, read.ledger))
}
