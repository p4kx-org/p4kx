// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::CatalogError;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

/// The electromagnetic and infrared signature blocks of an original state.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ResourceSignatureParams<'source> {
    source_type: SourcedValue<&'source str>,
    #[serde(rename = "EMSignature")]
    em_signature: SourcedValue<SignatureEntry<'source>>,
    #[serde(rename = "IRSignature")]
    ir_signature: SourcedValue<SignatureEntry<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ResourceSignatureParams<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let em_signature = source.declared_class("EMSignature", notes, SignatureEntry::read)?;
        let ir_signature = source.declared_class("IRSignature", notes, SignatureEntry::read)?;
        let additional_fields = source.authored_members_except([
            ("EMSignature", em_signature.state()),
            ("IRSignature", ir_signature.state()),
        ])?;
        Ok(Self {
            source_type: source.source_type_value()?,
            em_signature,
            ir_signature,
            additional_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct SignatureEntry<'source> {
    source_type: SourcedValue<&'source str>,
    nominal_signature: SourcedValue<ResourceFloat32>,
    decay_rate: SourcedValue<ResourceFloat32>,
    functionality_curve: SourcedValue<SourceArenaOutput<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> SignatureEntry<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let nominal_signature = source.single("nominalSignature", notes)?;
        let decay_rate = source.single("decayRate", notes)?;
        let functionality_curve =
            source.raw_strong_pointer_to("functionalityCurve", "BezierCurve", notes)?;
        let additional_fields = source.authored_members_except([
            ("nominalSignature", nominal_signature.state()),
            ("decayRate", decay_rate.state()),
            ("functionalityCurve", functionality_curve.state()),
        ])?;
        Ok(Self {
            source_type: source.source_type_value()?,
            nominal_signature,
            decay_rate,
            functionality_curve,
            additional_fields,
        })
    }
}
