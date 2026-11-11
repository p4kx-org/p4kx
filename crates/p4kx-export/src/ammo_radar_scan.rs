// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_projection::CatalogProjection;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_text_contract::GameTextRef;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::authored_nodes::AuthoredNodes;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;
use crate::{CatalogError, ExportAccountingError};

#[path = "ammo_radar_scan_value.rs"]
mod values;

use values::{SCAN_VALUE_TYPES, ScanValue};

const SCAN_DATA_TYPES: &[&str] = &[
    "ScanCustomData",
    "ScanCustomDataDirect",
    "ScanCustomDataTagToggle",
];

/// Ordered original procedures and their owner-local typed value nodes.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct EmbeddedScanInfo<'source> {
    scan_procedures: SourcedValue<Vec<SourcedValue<ScanProcedure<'source>>>>,
    values: Vec<ScanValue>,
}

impl<'source> EmbeddedScanInfo<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut queue = AuthoredNodes::default();
        let scan_procedures = source.class_array_to(
            "scanProcedures",
            "CustomScanProcedureParams",
            notes,
            |source, notes| ScanProcedure::read(source, projection, &mut queue, notes),
        )?;
        let values = queue.read(notes, |source, pending, notes| {
            ScanValue::read(source, pending, projection, notes)
        })?;
        Ok(Self {
            scan_procedures,
            values,
        })
    }
}

/// A custom procedure with original base settings and authored data slots.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ScanProcedure<'source> {
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    required_scan_tag: SourcedValue<Box<SourceRecordReference<'source>>>,
    emission_baseline: SourcedValue<ResourceFloat32>,
    runtime_duration: SourcedValue<ResourceFloat32>,
    allowed_in_ai_auto_scan: SourcedValue<bool>,
    allowed_in_focal_point_scan: SourcedValue<bool>,
    allowed_in_ping_broad_scan: SourcedValue<bool>,
    allowed_in_ping_focus_scan: SourcedValue<bool>,
    allowed_in_passive_scan: SourcedValue<bool>,
    custom_datas: SourcedValue<Vec<SourcedValue<ScanDatum<'source>>>>,
}

impl<'source> ScanProcedure<'source> {
    fn read<'reader, 'scope, 'read>(
        source: Observed<'reader, 'scope, 'read, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        queue: &mut AuthoredNodes<'reader, 'scope, 'read, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            required_scan_tag: source.record_link_to("requiredScanTag", "Tag", notes)?,
            emission_baseline: source.single("emissionBaseline", notes)?,
            runtime_duration: source.single("runtimeDuration", notes)?,
            allowed_in_ai_auto_scan: source.boolean("allowedInAIAutoScan", notes)?,
            allowed_in_focal_point_scan: source.boolean("allowedInFocalPointScan", notes)?,
            allowed_in_ping_broad_scan: source.boolean("allowedInPingBroadScan", notes)?,
            allowed_in_ping_focus_scan: source.boolean("allowedInPingFocusScan", notes)?,
            allowed_in_passive_scan: source.boolean("allowedInPassiveScan", notes)?,
            custom_datas: source.strong_pointer_array_to(
                "customDatas",
                "ScanCustomData",
                SCAN_DATA_TYPES,
                notes,
                |source, notes| ScanDatum::read(source, projection, queue, notes),
            )?,
        })
    }
}

/// One original scan data declaration and its selected typed payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ScanDatum<'source> {
    #[serde(flatten)]
    common: ScanDatumFields<'source>,
    #[serde(flatten)]
    value: ScanDataValue<'source>,
}

impl<'source> ScanDatum<'source> {
    fn read<'reader, 'scope, 'read>(
        source: Observed<'reader, 'scope, 'read, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        queue: &mut AuthoredNodes<'reader, 'scope, 'read, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let common = ScanDatumFields {
            required_tags: source.inline_class(
                "requiredTags",
                &["TagsDNFTerm"],
                notes,
                RadarTagTerm::read,
            )?,
            name: source.protected_locale("name", projection, notes)?,
            display_section: source.enumeration("displaySection", notes)?,
            add_to_boxout_display: source.boolean("addToBoxoutDisplay", notes)?,
            display_in_same_row_as_header: source.boolean("displayInSameRowAsHeader", notes)?,
        };
        let value = match source.source_type()? {
            "ScanCustomData" => ScanDataValue::Base,
            "ScanCustomDataDirect" => ScanDataValue::Direct {
                value: source.strong_pointer_to(
                    "value",
                    "ScanCustomValue",
                    SCAN_VALUE_TYPES,
                    notes,
                    |source, _| queue.push(source),
                )?,
            },
            "ScanCustomDataTagToggle" => ScanDataValue::TagToggle {
                default_value: source.strong_pointer_to(
                    "defaultValue",
                    "ScanCustomValue",
                    SCAN_VALUE_TYPES,
                    notes,
                    |source, _| queue.push(source),
                )?,
                tag_entries: source.class_array_to(
                    "tagEntries",
                    "ScanCustomDataTagEntry",
                    notes,
                    |source, notes| TagValue::read(source, queue, notes),
                )?,
            },
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        };
        Ok(Self { common, value })
    }
}

/// Authored display and tag settings shared by scan data alternatives.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ScanDatumFields<'source> {
    required_tags: SourcedValue<RadarTagTerm<'source>>,
    name: SourcedValue<GameTextRef>,
    display_section: SourcedValue<String>,
    add_to_boxout_display: SourcedValue<bool>,
    display_in_same_row_as_header: SourcedValue<bool>,
}

/// The typed authored value links selected by a scan data declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum ScanDataValue<'source> {
    Base,
    Direct {
        value: SourcedValue<u32>,
    },
    TagToggle {
        default_value: SourcedValue<u32>,
        tag_entries: SourcedValue<Vec<SourcedValue<TagValue<'source>>>>,
    },
}

/// One original tag selector and local typed scan-value reference.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TagValue<'source> {
    tags_to_match: SourcedValue<RadarTagTerm<'source>>,
    value: SourcedValue<u32>,
}

impl<'source> TagValue<'source> {
    fn read<'reader, 'scope, 'read>(
        source: Observed<'reader, 'scope, 'read, 'source>,
        queue: &mut AuthoredNodes<'reader, 'scope, 'read, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            tags_to_match: source.inline_class(
                "tagsToMatch",
                &["TagsDNFTerm"],
                notes,
                RadarTagTerm::read,
            )?,
            value: source.strong_pointer_to(
                "value",
                "ScanCustomValue",
                SCAN_VALUE_TYPES,
                notes,
                |source, _| queue.push(source),
            )?,
        })
    }
}

/// The exact positive and negative tag slots of one authored term.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct RadarTagTerm<'source> {
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    positive_tags: SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    negative_tags: SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>,
}

impl<'source> RadarTagTerm<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            positive_tags: source.record_link_array_to("positiveTags", "Tag", notes)?,
            negative_tags: source.record_link_array_to("negativeTags", "Tag", notes)?,
        })
    }
}
