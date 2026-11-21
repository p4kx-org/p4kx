// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_damage::{DamageChannels, DamageInfo, DamageParameters};
use super::authored_fields::Observed;
use crate::CatalogError;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

pub(super) const BASE: &str = "DamageBase";
pub(super) const INFO: &str = "DamageInfo";

/// The concrete authored vehicle armour multiplier and its original members.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum VehicleArmorDamage<'source> {
    /// The concrete base declaration.
    Base {
        /// Original type and additional members.
        #[serde(flatten)]
        source: ArmorDamageSource<'source>,
    },
    /// A concrete vector of damage words.
    Info {
        /// Original type, damage words and additional members.
        #[serde(flatten)]
        value: Box<ArmorDamageInfo<'source>>,
    },
    /// A concrete damage total and macro link.
    Parameters {
        /// Original type and additional members.
        #[serde(flatten)]
        source: ArmorDamageSource<'source>,
        /// Original total and macro link.
        #[serde(flatten)]
        value: Box<DamageParameters<'source>>,
    },
    /// An unclassified derived type retained with its original members.
    Unknown {
        /// Original type and all authored members.
        #[serde(flatten)]
        source: ArmorDamageSource<'source>,
    },
}

impl<'source> VehicleArmorDamage<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            BASE => Self::Base {
                source: ArmorDamageSource::read(source, [])?,
            },
            INFO => Self::Info {
                value: Box::new(ArmorDamageInfo::read(source, notes)?),
            },
            "DamageParams" => {
                let value = DamageParameters::read(source, notes)?;
                let source = ArmorDamageSource::read(source, value.projected())?;
                Self::Parameters {
                    source,
                    value: Box::new(value),
                }
            }
            _ => {
                source.note_retained_variant(notes)?;
                Self::Unknown {
                    source: ArmorDamageSource::read(source, [])?,
                }
            }
        })
    }
}

/// The shared six damage words and additional original members of a concrete Class.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
pub struct ArmorDamageInfo<'source> {
    #[serde(flatten)]
    source: ArmorDamageSource<'source>,
    #[serde(flatten)]
    values: DamageInfo,
}

impl<'source> ArmorDamageInfo<'source> {
    /// Borrows the original damage type and other authored fields.
    #[must_use]
    pub const fn source(&self) -> &ArmorDamageSource<'source> {
        &self.source
    }

    /// Returns the six original damage words.
    #[must_use]
    pub const fn channels(&self) -> DamageChannels<&SourcedValue<ResourceFloat32>> {
        self.values.channels()
    }

    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let values = DamageInfo::read(source, notes)?;
        let source = ArmorDamageSource::read(source, values.projected())?;
        Ok(Self { source, values })
    }
}

/// The concrete damage type and its remaining original members.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ArmorDamageSource<'source> {
    source_type: SourcedValue<&'source str>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ArmorDamageSource<'source> {
    /// Borrows the concrete authored type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the remaining original members.
    #[must_use]
    pub const fn additional_fields(&self) -> &SourceArenaOutput<'source> {
        &self.additional_fields
    }

    fn read(
        source: Observed<'_, '_, '_, 'source>,
        projected: impl IntoIterator<Item = (&'static str, SourceValueState)>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            source_type: source.source_type_value()?,
            additional_fields: source.authored_members_except(projected)?,
        })
    }
}
