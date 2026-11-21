// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::authored_fields::Observed;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;

/// The exact authored path of a geometry or material asset.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct GeometryAsset<'source> {
    path: SourcedValue<&'source str>,
}

impl<'source> GeometryAsset<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            path: source.borrowed_string("path", notes)?,
        })
    }
}

/// An authored palette Reference and its optional local override.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct GeometryPalette<'source> {
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    root_record: SourcedValue<Box<SourceRecordReference<'source>>>,
    child_path: SourcedValue<&'source str>,
    swizzle_override: SourcedValue<GeometryPaletteSwizzle>,
}

impl<'source> GeometryPalette<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            root_record: source.record_link_to("RootRecord", "TintPaletteTree", notes)?,
            child_path: source.borrowed_string("ChildPath", notes)?,
            swizzle_override: source.strong_pointer_to(
                "SwizzleOverride",
                "TintPaletteSwizzle",
                &["TintPaletteSwizzle"],
                notes,
                GeometryPaletteSwizzle::read,
            )?,
        })
    }
}

/// The three stored palette-channel selectors.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct GeometryPaletteSwizzle {
    primary: SourcedValue<i32>,
    secondary: SourcedValue<i32>,
    tertiary: SourcedValue<i32>,
}

impl GeometryPaletteSwizzle {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            primary: source.int32("primary", notes)?,
            secondary: source.int32("secondary", notes)?,
            tertiary: source.int32("tertiary", notes)?,
        })
    }
}
