// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::authored_link::HealthLink;
use crate::CatalogError;
use crate::catalog_v3::catalog_item_components::source_getters;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

pub(super) const DAMAGE_TYPES: &[&str] = &["DamageBase", "DamageInfo", "DamageParams"];

/// The authored damage alternatives of one `DamageBase` declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum AuthoredDamage<'source> {
    /// The base damage declaration.
    Base,
    /// An explicit set of damage channels.
    Info {
        /// The original per-channel damage values.
        value: Box<DamageInfo>,
    },
    /// A damage macro and declared total.
    Parameters {
        /// The original macro link and total.
        value: Box<DamageParameters<'source>>,
    },
}

impl<'source> AuthoredDamage<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            "DamageBase" => Self::Base,
            "DamageInfo" => Self::Info {
                value: Box::new(DamageInfo::read(source, notes)?),
            },
            "DamageParams" => Self::Parameters {
                value: Box::new(DamageParameters::read(source, notes)?),
            },
            _ => return Err(crate::ExportAccountingError::ConflictingSourceDatum.into()),
        })
    }
}

/// Exact original damage words by declared type.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct DamageInfo {
    physical: SourcedValue<ResourceFloat32>,
    energy: SourcedValue<ResourceFloat32>,
    distortion: SourcedValue<ResourceFloat32>,
    thermal: SourcedValue<ResourceFloat32>,
    biochemical: SourcedValue<ResourceFloat32>,
    stun: SourcedValue<ResourceFloat32>,
}

impl DamageInfo {
    pub(super) fn from_authored<'damage>(
        damage: &'damage AuthoredDamage<'_>,
    ) -> Option<&'damage Self> {
        match damage {
            AuthoredDamage::Info { value } => Some(value),
            _ => None,
        }
    }

    /// Borrows each of the six authored damage channels.
    #[must_use]
    pub const fn channels(&self) -> DamageChannels<&SourcedValue<ResourceFloat32>> {
        DamageChannels {
            physical: &self.physical,
            energy: &self.energy,
            distortion: &self.distortion,
            thermal: &self.thermal,
            biochemical: &self.biochemical,
            stun: &self.stun,
        }
    }

    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            physical: source.single("DamagePhysical", notes)?,
            energy: source.single("DamageEnergy", notes)?,
            distortion: source.single("DamageDistortion", notes)?,
            thermal: source.single("DamageThermal", notes)?,
            biochemical: source.single("DamageBiochemical", notes)?,
            stun: source.single("DamageStun", notes)?,
        })
    }

    pub(super) const fn projected(&self) -> [(&'static str, SourceValueState); 6] {
        [
            ("DamagePhysical", self.physical.state()),
            ("DamageEnergy", self.energy.state()),
            ("DamageDistortion", self.distortion.state()),
            ("DamageThermal", self.thermal.state()),
            ("DamageBiochemical", self.biochemical.state()),
            ("DamageStun", self.stun.state()),
        ]
    }
}

/// A fixed vector keyed by the original damage channels.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
pub struct DamageChannels<T> {
    physical: T,
    energy: T,
    distortion: T,
    thermal: T,
    biochemical: T,
    stun: T,
}

impl<T> DamageChannels<T> {
    /// Borrows the physical damage channel.
    #[must_use]
    pub const fn physical(&self) -> &T {
        &self.physical
    }

    /// Borrows the energy damage channel.
    #[must_use]
    pub const fn energy(&self) -> &T {
        &self.energy
    }

    /// Borrows the distortion damage channel.
    #[must_use]
    pub const fn distortion(&self) -> &T {
        &self.distortion
    }

    /// Borrows the thermal damage channel.
    #[must_use]
    pub const fn thermal(&self) -> &T {
        &self.thermal
    }

    /// Borrows the biochemical damage channel.
    #[must_use]
    pub const fn biochemical(&self) -> &T {
        &self.biochemical
    }

    /// Borrows the stun damage channel.
    #[must_use]
    pub const fn stun(&self) -> &T {
        &self.stun
    }

    pub(crate) const fn as_ref(&self) -> DamageChannels<&T> {
        DamageChannels {
            physical: &self.physical,
            energy: &self.energy,
            distortion: &self.distortion,
            thermal: &self.thermal,
            biochemical: &self.biochemical,
            stun: &self.stun,
        }
    }

    pub(crate) fn try_map<U, E>(
        self,
        mut apply: impl FnMut(T) -> Result<U, E>,
    ) -> Result<DamageChannels<U>, E> {
        Ok(DamageChannels {
            physical: apply(self.physical)?,
            energy: apply(self.energy)?,
            distortion: apply(self.distortion)?,
            thermal: apply(self.thermal)?,
            biochemical: apply(self.biochemical)?,
            stun: apply(self.stun)?,
        })
    }
}

/// The original macro link and declared total.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct DamageParameters<'source> {
    damage_macro: SourcedValue<HealthLink<'source>>,
    damage_total: SourcedValue<ResourceFloat32>,
}

impl<'source> DamageParameters<'source> {
    /// Borrows the original macro link.
    #[must_use]
    pub const fn damage_macro(&self) -> &SourcedValue<HealthLink<'source>> {
        &self.damage_macro
    }

    /// Borrows the original total.
    #[must_use]
    pub const fn damage_total(&self) -> &SourcedValue<ResourceFloat32> {
        &self.damage_total
    }

    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            damage_macro: source.link("damageMacro", notes)?,
            damage_total: source.single("damageTotal", notes)?,
        })
    }

    pub(super) const fn projected(&self) -> [(&'static str, SourceValueState); 2] {
        [
            ("damageMacro", self.damage_macro.state()),
            ("damageTotal", self.damage_total.state()),
        ]
    }
}

/// Three original Single coordinates of an authored Vec3.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct DamageVector {
    x: SourcedValue<ResourceFloat32>,
    y: SourcedValue<ResourceFloat32>,
    z: SourcedValue<ResourceFloat32>,
}

impl DamageVector {
    source_getters! {
        /// Borrows the authored X coordinate.
        x: SourcedValue<ResourceFloat32>,
        /// Borrows the authored Y coordinate.
        y: SourcedValue<ResourceFloat32>,
        /// Borrows the authored Z coordinate.
        z: SourcedValue<ResourceFloat32>,
    }

    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            x: source.single("x", notes)?,
            y: source.single("y", notes)?,
            z: source.single("z", notes)?,
        })
    }
}
