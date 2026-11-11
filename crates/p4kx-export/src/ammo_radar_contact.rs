// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::signature::{BASE_TYPES, BaseSignature, EMISSION_TYPES, EmissionModifier};
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::authored_damage::DamageVector;
use crate::catalog_v3::authored_fields::Observed;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;

const CROSS_SECTION_TYPES: &[&str] = &[
    "SSCSignatureSystemCrossSectionParams",
    "SSCSignatureSystemActorCrossSectionParams",
    "SSCSignatureSystemManualCrossSectionParams",
    "SSCSignatureSystemSDFCrossSectionParams",
];

/// Original radar-contact, signature and scan-bound declarations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct RadarContact<'source> {
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    contact_type: SourcedValue<Box<SourceRecordReference<'source>>>,
    base_signature_params: SourcedValue<BaseSignature<'source>>,
    cross_section_params: SourcedValue<CrossSection>,
    emission_modifier_params: SourcedValue<EmissionModifier>,
    death_params: SourcedValue<SignatureDeath>,
    scan_bounds: SourcedValue<ScanBounds>,
    room_params: SourcedValue<RoomParameters>,
}

impl<'source> RadarContact<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            contact_type: source.record_link_to("contactType", "RadarContactTypeEntry", notes)?,
            base_signature_params: source.strong_pointer_to(
                "baseSignatureParams",
                "SSCSignatureParamsBase",
                BASE_TYPES,
                notes,
                BaseSignature::read,
            )?,
            cross_section_params: source.strong_pointer_to(
                "crossSectionParams",
                "SSCSignatureSystemCrossSectionParams",
                CROSS_SECTION_TYPES,
                notes,
                CrossSection::read,
            )?,
            emission_modifier_params: source.strong_pointer_to(
                "emissionModifierParams",
                "SSCSignatureEmissionBaseModifier",
                EMISSION_TYPES,
                notes,
                EmissionModifier::read,
            )?,
            death_params: source.strong_pointer_to(
                "deathParams",
                "SCSignatureDeathParams",
                &["SCSignatureDeathParams"],
                notes,
                SignatureDeath::read,
            )?,
            scan_bounds: source.strong_pointer_to(
                "scanBounds",
                "SSCSignatureSystemScanBounds",
                &["SSCSignatureSystemScanBounds"],
                notes,
                ScanBounds::read,
            )?,
            room_params: source.inline_class(
                "roomParams",
                &["SCSignatureSystemRoomParams"],
                notes,
                RoomParameters::read,
            )?,
        })
    }
}

/// An authored cross-section alternative.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum CrossSection {
    Base,
    Actor,
    Manual {
        cross_section: SourcedValue<Box<DamageVector>>,
    },
    Sdf,
}

impl CrossSection {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            "SSCSignatureSystemCrossSectionParams" => Self::Base,
            "SSCSignatureSystemActorCrossSectionParams" => Self::Actor,
            "SSCSignatureSystemManualCrossSectionParams" => Self::Manual {
                cross_section: source.inline_class(
                    "crossSection",
                    &["Vec3"],
                    notes,
                    |source, notes| DamageVector::read(source, notes).map(Box::new),
                )?,
            },
            "SSCSignatureSystemSDFCrossSectionParams" => Self::Sdf,
            _ => return Err(crate::ExportAccountingError::ConflictingSourceDatum.into()),
        })
    }
}

/// Original modifier selected by the death-parameter pointer.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct SignatureDeath {
    death_emission_modifier_params: SourcedValue<EmissionModifier>,
}

impl SignatureDeath {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            death_emission_modifier_params: source.strong_pointer_to(
                "deathEmissionModifierParams",
                "SSCSignatureEmissionBaseModifier",
                EMISSION_TYPES,
                notes,
                EmissionModifier::read,
            )?,
        })
    }
}

/// Original local minimum, maximum and rotation of scan bounds.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ScanBounds {
    min: SourcedValue<DamageVector>,
    max: SourcedValue<DamageVector>,
    local_rotation: SourcedValue<DamageVector>,
}

impl ScanBounds {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            min: source.inline_class("min", &["Vec3"], notes, DamageVector::read)?,
            max: source.inline_class("max", &["Vec3"], notes, DamageVector::read)?,
            local_rotation: source.inline_class(
                "localRotation",
                &["Deg3"],
                notes,
                DamageVector::read,
            )?,
        })
    }
}

/// Original interior/exterior check setting.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct RoomParameters {
    ignore_interior_vs_exterior_check: SourcedValue<bool>,
}

impl RoomParameters {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            ignore_interior_vs_exterior_check: source
                .boolean("ignoreInteriorVsExteriorCheck", notes)?,
        })
    }
}
