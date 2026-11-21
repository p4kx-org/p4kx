// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_damage::{DamageInfo, DamageParameters};
use crate::catalog_v3::authored_fields::Observed;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

pub(super) const BASE: &str = "DamageBase";

/// The concrete original beam damage declaration and its untouched members.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct BeamDamage<'source> {
    source_type: SourcedValue<&'source str>,
    #[serde(flatten)]
    inputs: BeamDamageInputs<'source>,
    authored: SourceArenaOutput<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
enum BeamDamageInputs<'source> {
    Base,
    Channels(Box<DamageInfo>),
    Parameters(Box<DamageParameters<'source>>),
    Authored,
}

impl<'source> BeamDamage<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let (inputs, authored) = match source.source_type()? {
            BASE => (BeamDamageInputs::Base, source.authored_members()?),
            "DamageInfo" => {
                let fields = DamageInfo::read(source, notes)?;
                let authored = source.authored_members_except(fields.projected())?;
                (BeamDamageInputs::Channels(Box::new(fields)), authored)
            }
            "DamageParams" => {
                let fields = DamageParameters::read(source, notes)?;
                let authored = source.authored_members_except(fields.projected())?;
                (BeamDamageInputs::Parameters(Box::new(fields)), authored)
            }
            _ => {
                source.note_retained_variant(notes)?;
                (BeamDamageInputs::Authored, source.authored_members()?)
            }
        };
        Ok(Self {
            source_type: source.source_type_value()?,
            inputs,
            authored,
        })
    }
}
