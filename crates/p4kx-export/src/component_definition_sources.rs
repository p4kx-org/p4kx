// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, RecordTree};

use super::catalog_independent_configurations::electrical_behavior::{
    ElectricalBehavior, SOURCE_TYPE as ELECTRICAL_BEHAVIOR,
};
use super::catalog_independent_configurations::{
    CONSUMABLE_SUBTYPE, CONSUMABLE_TYPE, ConsumableSubtype, ConsumableType,
    DAMAGE_RESISTANCE_MACRO, DamageResistanceMacro, HEALTH_MODIFIER, MINING_LASER_GLOBAL,
    MiningLaserGlobalDefinition, TurretHealthModifier, observe_modifier_types,
};
use super::referenced_definition_read;
use crate::CatalogError;
use crate::catalog_projection::{CatalogProjection, TextOwner};
use crate::catalog_queries::CatalogRecordSelection;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::export_field_inspection::ConfigurationInspection;
use crate::export_ledger::ExportLedger;
use crate::record_observation::RecordObservation;

pub(crate) trait Definition {
    type Row<'source>;
    const SOURCE_TYPE: &'static str;
    const TEXT_OWNER: Option<TextOwner> = None;

    fn observe_types<'source>(
        _forge: &'source DataForge<'source>,
        _projection: &CatalogProjection<'_, 'source>,
        _selection: CatalogRecordSelection,
        _cancel: Option<&CancelToken>,
    ) -> Result<(Vec<&'source str>, ExportLedger), CatalogError> {
        Ok((vec![Self::SOURCE_TYPE], ExportLedger::default()))
    }

    fn project<'source>(
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Self::Row<'source>>, CatalogError>;
}

macro_rules! direct_definition {
    ($marker:ident, $row:ident, $source:ident, $inspection:ident) => {
        pub(crate) struct $marker;

        impl Definition for $marker {
            type Row<'source> = $row<'source>;
            const SOURCE_TYPE: &'static str = $source;

            fn project<'source>(
                tree: &RecordTree<'source>,
                projection: &CatalogProjection<'_, 'source>,
                admission: &RecordObservation<'_, 'source>,
                cancel: Option<&CancelToken>,
            ) -> Result<ScopedSourceRead<Self::Row<'source>>, CatalogError> {
                referenced_definition_read::read_selected(
                    tree,
                    projection.original(),
                    admission,
                    ConfigurationInspection::$inspection,
                    cancel,
                    $row::read,
                )
            }
        }
    };
}

direct_definition!(
    ElectricalBehaviors,
    ElectricalBehavior,
    ELECTRICAL_BEHAVIOR,
    ElectricalBehavior
);
direct_definition!(
    ArmorMacros,
    DamageResistanceMacro,
    DAMAGE_RESISTANCE_MACRO,
    DamageResistanceMacro
);
direct_definition!(
    MiningLaserGlobals,
    MiningLaserGlobalDefinition,
    MINING_LASER_GLOBAL,
    MiningLaserGlobal
);
direct_definition!(
    ConsumableTypes,
    ConsumableType,
    CONSUMABLE_TYPE,
    ConsumableDefinition
);

pub(crate) struct ConsumableSubtypes;

impl Definition for ConsumableSubtypes {
    type Row<'source> = ConsumableSubtype<'source>;
    const SOURCE_TYPE: &'static str = CONSUMABLE_SUBTYPE;
    const TEXT_OWNER: Option<TextOwner> = Some(TextOwner::ConsumableSubtypes);

    fn project<'source>(
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Self::Row<'source>>, CatalogError> {
        referenced_definition_read::read_selected(
            tree,
            projection.original(),
            admission,
            ConfigurationInspection::ConsumableDefinition,
            cancel,
            |_, header, fields, notes| ConsumableSubtype::read(header, fields, projection, notes),
        )
    }
}

pub(crate) struct TurretModifiers;

impl Definition for TurretModifiers {
    type Row<'source> = TurretHealthModifier<'source>;
    const SOURCE_TYPE: &'static str = HEALTH_MODIFIER;

    fn observe_types<'source>(
        forge: &'source DataForge<'source>,
        projection: &CatalogProjection<'_, 'source>,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
    ) -> Result<(Vec<&'source str>, ExportLedger), CatalogError> {
        if !matches!(selection, CatalogRecordSelection::All) {
            let mut needs_discovery = false;
            selection.for_each(forge, cancel, |record| {
                needs_discovery |= record.type_name().is_ok_and(|name| name != HEALTH_MODIFIER);
                Ok(())
            })?;
            if !needs_discovery {
                return Ok((vec![HEALTH_MODIFIER], ExportLedger::default()));
            }
        }
        observe_modifier_types(forge, projection.original(), cancel)
    }

    fn project<'source>(
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Self::Row<'source>>, CatalogError> {
        referenced_definition_read::read_selected(
            tree,
            projection.original(),
            admission,
            ConfigurationInspection::TurretHealthModifier,
            cancel,
            TurretHealthModifier::read,
        )
    }
}
