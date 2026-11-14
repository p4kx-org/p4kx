// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_damage::{DamageInfo, DamageParameters};
use super::authored_fields::Observed;
use crate::CatalogError;
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
pub(super) enum VehicleArmorDamage<'source> {
    Base {
        #[serde(flatten)]
        source: ArmorDamageSource<'source>,
    },
    Info {
        #[serde(flatten)]
        value: Box<ArmorDamageInfo<'source>>,
    },
    Parameters {
        #[serde(flatten)]
        source: ArmorDamageSource<'source>,
        #[serde(flatten)]
        value: Box<DamageParameters<'source>>,
    },
    Unknown {
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
pub(super) struct ArmorDamageInfo<'source> {
    #[serde(flatten)]
    source: ArmorDamageSource<'source>,
    #[serde(flatten)]
    values: DamageInfo,
}

impl<'source> ArmorDamageInfo<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let values = DamageInfo::read(source, notes)?;
        let source = ArmorDamageSource::read(source, values.projected())?;
        Ok(Self { source, values })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ArmorDamageSource<'source> {
    source_type: SourcedValue<&'source str>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ArmorDamageSource<'source> {
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
