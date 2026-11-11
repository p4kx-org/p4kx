// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::observed::Observed;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;
use crate::{CatalogError, ExportAccountingError};

const BASE: &str = "DamageResistanceBase";
const INLINE: &str = "DamageResistance";
const REFERENCE: &str = "DamageResistanceReference";

/// The actual concrete resistance form owned by the Health component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum DurabilityResistances<'source> {
    Base,
    Resistances(Box<ResistanceSet>),
    Reference {
        #[schemars(with = "SourcedValue<CatalogRecordReference>")]
        damage_resistance_record: SourcedValue<Box<SourceRecordReference<'source>>>,
    },
}

impl<'source> DurabilityResistances<'source> {
    pub(super) const TYPES: &[&str] = &[BASE, INLINE, REFERENCE];

    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            BASE => Self::Base,
            INLINE => Self::Resistances(Box::new(ResistanceSet::read(source, notes)?)),
            REFERENCE => Self::Reference {
                damage_resistance_record: source.record_link("damageResistanceRecord", notes)?,
            },
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        })
    }
}

/// The six named resistance entries and the original melee flag.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ResistanceSet {
    ignore_melee_damage: SourcedValue<bool>,
    physical: SourcedValue<DurabilityResistanceEntry>,
    energy: SourcedValue<DurabilityResistanceEntry>,
    distortion: SourcedValue<DurabilityResistanceEntry>,
    thermal: SourcedValue<DurabilityResistanceEntry>,
    biochemical: SourcedValue<DurabilityResistanceEntry>,
    stun: SourcedValue<DurabilityResistanceEntry>,
}

impl ResistanceSet {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            ignore_melee_damage: source.boolean("IgnoreMeleeDamage", notes)?,
            physical: source.owned(
                "PhysicalResistance",
                &["DamageResistanceEntry"],
                notes,
                DurabilityResistanceEntry::read,
            )?,
            energy: source.owned(
                "EnergyResistance",
                &["DamageResistanceEntry"],
                notes,
                DurabilityResistanceEntry::read,
            )?,
            distortion: source.owned(
                "DistortionResistance",
                &["DamageResistanceEntry"],
                notes,
                DurabilityResistanceEntry::read,
            )?,
            thermal: source.owned(
                "ThermalResistance",
                &["DamageResistanceEntry"],
                notes,
                DurabilityResistanceEntry::read,
            )?,
            biochemical: source.owned(
                "BiochemicalResistance",
                &["DamageResistanceEntry"],
                notes,
                DurabilityResistanceEntry::read,
            )?,
            stun: source.owned(
                "StunResistance",
                &["DamageResistanceEntry"],
                notes,
                DurabilityResistanceEntry::read,
            )?,
        })
    }
}

/// Four original fields of one named resistance entry.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct DurabilityResistanceEntry {
    multiplier: SourcedValue<ResourceFloat32>,
    threshold: SourcedValue<ResourceFloat32>,
    damage_cap: SourcedValue<ResourceFloat32>,
    damage_cap_limit: SourcedValue<ResourceFloat32>,
}

impl DurabilityResistanceEntry {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            multiplier: source.single("Multiplier", notes)?,
            threshold: source.single("Threshold", notes)?,
            damage_cap: source.single("DamageCap", notes)?,
            damage_cap_limit: source.single("DamageCapLimit", notes)?,
        })
    }
}
