// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
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
pub(in crate::catalog_v3) const INLINE: &str = "DamageResistance";
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
pub struct ResistanceSet {
    ignore_melee_damage: SourcedValue<bool>,
    physical: SourcedValue<DurabilityResistanceEntry>,
    energy: SourcedValue<DurabilityResistanceEntry>,
    distortion: SourcedValue<DurabilityResistanceEntry>,
    thermal: SourcedValue<DurabilityResistanceEntry>,
    biochemical: SourcedValue<DurabilityResistanceEntry>,
    stun: SourcedValue<DurabilityResistanceEntry>,
}

impl ResistanceSet {
    /// Borrows the original melee exclusion flag.
    #[must_use]
    pub const fn ignore_melee_damage(&self) -> &SourcedValue<bool> {
        &self.ignore_melee_damage
    }

    /// Borrows the physical channel.
    #[must_use]
    pub const fn physical(&self) -> &SourcedValue<DurabilityResistanceEntry> {
        &self.physical
    }

    /// Borrows the energy channel.
    #[must_use]
    pub const fn energy(&self) -> &SourcedValue<DurabilityResistanceEntry> {
        &self.energy
    }

    /// Borrows the distortion channel.
    #[must_use]
    pub const fn distortion(&self) -> &SourcedValue<DurabilityResistanceEntry> {
        &self.distortion
    }

    /// Borrows the thermal channel.
    #[must_use]
    pub const fn thermal(&self) -> &SourcedValue<DurabilityResistanceEntry> {
        &self.thermal
    }

    /// Borrows the biochemical channel.
    #[must_use]
    pub const fn biochemical(&self) -> &SourcedValue<DurabilityResistanceEntry> {
        &self.biochemical
    }

    /// Borrows the stun channel.
    #[must_use]
    pub const fn stun(&self) -> &SourcedValue<DurabilityResistanceEntry> {
        &self.stun
    }

    pub(in crate::catalog_v3) fn read(
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

impl ResistanceSet {
    pub(in crate::catalog_v3) const fn projected(&self) -> [(&'static str, SourceValueState); 7] {
        [
            ("IgnoreMeleeDamage", self.ignore_melee_damage.state()),
            ("PhysicalResistance", self.physical.state()),
            ("EnergyResistance", self.energy.state()),
            ("DistortionResistance", self.distortion.state()),
            ("ThermalResistance", self.thermal.state()),
            ("BiochemicalResistance", self.biochemical.state()),
            ("StunResistance", self.stun.state()),
        ]
    }
}

/// Four original fields of one named resistance entry.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct DurabilityResistanceEntry {
    multiplier: SourcedValue<ResourceFloat32>,
    threshold: SourcedValue<ResourceFloat32>,
    damage_cap: SourcedValue<ResourceFloat32>,
    damage_cap_limit: SourcedValue<ResourceFloat32>,
}

impl DurabilityResistanceEntry {
    /// Borrows the authored multiplier.
    #[must_use]
    pub const fn multiplier(&self) -> &SourcedValue<ResourceFloat32> {
        &self.multiplier
    }

    /// Borrows the authored threshold.
    #[must_use]
    pub const fn threshold(&self) -> &SourcedValue<ResourceFloat32> {
        &self.threshold
    }

    /// Borrows the authored damage cap.
    #[must_use]
    pub const fn damage_cap(&self) -> &SourcedValue<ResourceFloat32> {
        &self.damage_cap
    }

    /// Borrows the authored damage-cap limit.
    #[must_use]
    pub const fn damage_cap_limit(&self) -> &SourcedValue<ResourceFloat32> {
        &self.damage_cap_limit
    }

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
