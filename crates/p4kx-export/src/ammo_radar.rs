// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::authored_damage::DamageVector;
use crate::catalog_v3::authored_fields::Observed;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;

#[path = "ammo_radar_contact.rs"]
mod contact;
#[path = "ammo_radar_scan.rs"]
mod scan;
#[path = "ammo_radar_signature.rs"]
mod signature;

use contact::RadarContact;
use scan::EmbeddedScanInfo;

/// Original signature-system configuration stored by one Ammo definition.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AmmoRadar<'source> {
    binding_url_prefix: SourcedValue<&'source str>,
    radar_properties: SourcedValue<RadarContact<'source>>,
    audio_params: SourcedValue<RadarAudio<'source>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    scan_custom_data: SourcedValue<Box<SourceRecordReference<'source>>>,
    embedded_scan_info: SourcedValue<EmbeddedScanInfo<'source>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    scan_display_layout_override: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    detection_tags: SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>,
    is_overridden: SourcedValue<bool>,
    overridden_size: SourcedValue<DamageVector>,
    enable_detection_on_item_port: SourcedValue<bool>,
    ignore_highlight_when_detector_inside_bounds: SourcedValue<bool>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    linked_objectives: SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>,
    ignore_highlight_when_no_linked_or_active_objectives: SourcedValue<bool>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    priority_boxout_tag: SourcedValue<Box<SourceRecordReference<'source>>>,
    is_object_of_interest: SourcedValue<bool>,
}

impl<'source> AmmoRadar<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            binding_url_prefix: source.borrowed_string("bindingURLPrefix", notes)?,
            radar_properties: source.strong_pointer_to(
                "radarProperties",
                "SSCRadarContactProperites",
                &["SSCRadarContactProperites"],
                notes,
                RadarContact::read,
            )?,
            audio_params: source.strong_pointer_to(
                "audioParams",
                "SSCSignatureSystemAudioParams",
                &["SSCSignatureSystemAudioParams"],
                notes,
                RadarAudio::read,
            )?,
            scan_custom_data: source.record_link_to(
                "scanCustomData",
                "ScanCustomDataDef",
                notes,
            )?,
            embedded_scan_info: source.strong_pointer_to(
                "embeddedScanInfo",
                "ScanCustomDataInfo",
                &["ScanCustomDataInfo"],
                notes,
                |source, notes| EmbeddedScanInfo::read(source, projection, notes),
            )?,
            scan_display_layout_override: source.record_link_to(
                "scanDisplayLayoutOverride",
                "ScanDisplayLayoutParams",
                notes,
            )?,
            detection_tags: source.record_link_array_to("detectionTags", "Tag", notes)?,
            is_overridden: source.boolean("isOverridden", notes)?,
            overridden_size: source.inline_class(
                "overriddenSize",
                &["Vec3"],
                notes,
                DamageVector::read,
            )?,
            enable_detection_on_item_port: source.boolean("enableDetectionOnItemPort", notes)?,
            ignore_highlight_when_detector_inside_bounds: source
                .boolean("ignoreHighlightWhenDetectorInsideBounds", notes)?,
            linked_objectives: source.record_link_array_to(
                "linkedObjectives",
                "MissionObjective",
                notes,
            )?,
            ignore_highlight_when_no_linked_or_active_objectives: source
                .boolean("ignoreHighlightWhenNoLinkedOrActiveObjectives", notes)?,
            priority_boxout_tag: source.record_link_to("priorityBoxoutTag", "Tag", notes)?,
            is_object_of_interest: source.boolean("isObjectOfInterest", notes)?,
        })
    }
}

/// Ordered original ruleset links for audio signature behavior.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct RadarAudio<'source> {
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    rulesets: SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>,
}

impl<'source> RadarAudio<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            rulesets: source.record_link_array_to(
                "rulesets",
                "SSCSignatureSystemAudioRuleset",
                notes,
            )?,
        })
    }
}
