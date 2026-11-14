// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::authored_fields::Observed;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;

pub(super) const BASE_TYPES: &[&str] = &[
    "SSCSignatureParamsBase",
    "SSCSignatureSystemBaseSignatureParams",
];
pub(super) const EMISSION_TYPES: &[&str] = &[
    "SSCSignatureEmissionBaseModifier",
    "SSCSignatureEmissionModifier",
];

/// Original base-signature parameters and authored category/tag slots.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum BaseSignature<'source> {
    Base,
    Signature {
        value: Box<SignatureParameters<'source>>,
    },
}

impl<'source> BaseSignature<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            "SSCSignatureParamsBase" => Self::Base,
            "SSCSignatureSystemBaseSignatureParams" => Self::Signature {
                value: Box::new(SignatureParameters::read(source, notes)?),
            },
            _ => return Err(crate::ExportAccountingError::ConflictingSourceDatum.into()),
        })
    }
}

/// Original emission modifier and repeated authored entries.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum EmissionModifier {
    Base,
    Modifier {
        modifier_entries: SourcedValue<Vec<SourcedValue<ModifierEntry>>>,
    },
}

impl EmissionModifier {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            "SSCSignatureEmissionBaseModifier" => Self::Base,
            "SSCSignatureEmissionModifier" => Self::Modifier {
                modifier_entries: source.class_array_to(
                    "modifierEntries",
                    "SModifierSignaturesEntry",
                    notes,
                    ModifierEntry::read,
                )?,
            },
            _ => return Err(crate::ExportAccountingError::ConflictingSourceDatum.into()),
        })
    }
}

/// Original signature word arrays and their selector entries.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct SignatureParameters<'source> {
    signatures: SourcedValue<Vec<SourcedValue<ResourceFloat32>>>,
    signature_categories: SourcedValue<Vec<SourcedValue<SignatureCategory<'source>>>>,
    tagged_signatures: SourcedValue<Vec<SourcedValue<TaggedSignature<'source>>>>,
}

impl<'source> SignatureParameters<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            signatures: source.single_array("signatures", notes)?,
            signature_categories: source.class_array_to(
                "signatureCategories",
                "SSignaturesCategoryEntry",
                notes,
                SignatureCategory::read,
            )?,
            tagged_signatures: source.class_array_to(
                "taggedSignatures",
                "STaggedSignaturesEntry",
                notes,
                TaggedSignature::read,
            )?,
        })
    }
}

/// Original tag filter, signature words and signature categories.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TaggedSignature<'source> {
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    tag_to_match: SourcedValue<Box<SourceRecordReference<'source>>>,
    signatures: SourcedValue<Vec<SourcedValue<ResourceFloat32>>>,
    signature_categories: SourcedValue<Vec<SourcedValue<SignatureCategory<'source>>>>,
}

impl<'source> TaggedSignature<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            tag_to_match: source.record_link_to("tagToMatch", "Tag", notes)?,
            signatures: source.single_array("signatures", notes)?,
            signature_categories: source.class_array_to(
                "signatureCategories",
                "SSignaturesCategoryEntry",
                notes,
                SignatureCategory::read,
            )?,
        })
    }
}

/// One original signature/category pair.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct SignatureCategory<'source> {
    #[serde(rename = "signatureType")]
    kind: SourcedValue<String>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    #[serde(rename = "signatureCategory")]
    category: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[serde(rename = "signatureValue")]
    value: SourcedValue<ResourceFloat32>,
}

impl<'source> SignatureCategory<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            kind: source.enumeration("signatureType", notes)?,
            category: source.record_link_to(
                "signatureCategory",
                "RadarSignatureCategoryEntry",
                notes,
            )?,
            value: source.single("signatureValue", notes)?,
        })
    }
}

/// One original modifier kind and stored value.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ModifierEntry {
    modifier_type: SourcedValue<String>,
    value: SourcedValue<ResourceFloat32>,
}

impl ModifierEntry {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            modifier_type: source.enumeration("modifierType", notes)?,
            value: source.single("value", notes)?,
        })
    }
}
