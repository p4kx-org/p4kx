// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;
use crate::source_value_catalog::SourceRecordReference;

const PROPERTIES_TYPE: &str = "SSCRadarContactProperites";

/// Authored starmap radar properties and signature inputs.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct RadarProperties<'source> {
    /// The original concrete type.
    pub source_type: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    /// The original radar contact type reference.
    pub contact_type: SourcedValue<Box<SourceRecordReference<'source>>>,
    /// The original base signature fields.
    pub base_signature_params: SourcedValue<SourceArenaOutput<'source>>,
    /// The original cross-section fields.
    pub cross_section_params: SourcedValue<SourceArenaOutput<'source>>,
    /// The original emission modifier fields.
    pub emission_modifier_params: SourcedValue<SourceArenaOutput<'source>>,
    /// The original death settings.
    pub death_params: SourcedValue<SourceArenaOutput<'source>>,
    /// The original scan bounds fields.
    pub scan_bounds: SourcedValue<SourceArenaOutput<'source>>,
    /// The authored room settings.
    pub room_params: SourcedValue<RadarRoomParams<'source>>,
    /// Additional original fields.
    pub authored_fields: SourceArenaOutput<'source>,
}

impl<'source> RadarProperties<'source> {
    pub(super) fn read_field(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Self>, CatalogError> {
        fields.strong_pointer_family_to("radarProperties", PROPERTIES_TYPE, notes, Self::read)
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let contact_type = fields.record_link_to("contactType", "RadarContactTypeEntry", notes)?;
        let base_signature_params =
            fields.raw_strong_pointer_to("baseSignatureParams", "SSCSignatureParamsBase", notes)?;
        let cross_section_params = fields.raw_strong_pointer_to(
            "crossSectionParams",
            "SSCSignatureSystemCrossSectionParams",
            notes,
        )?;
        let emission_modifier_params = fields.raw_strong_pointer_to(
            "emissionModifierParams",
            "SSCSignatureEmissionBaseModifier",
            notes,
        )?;
        let death_params =
            fields.raw_strong_pointer_to("deathParams", "SCSignatureDeathParams", notes)?;
        let scan_bounds =
            fields.raw_strong_pointer_to("scanBounds", "SSCSignatureSystemScanBounds", notes)?;
        let room_params = fields.declared_class_to(
            "roomParams",
            "SCSignatureSystemRoomParams",
            notes,
            RadarRoomParams::read,
        )?;
        let authored_fields = fields.authored_members_except([
            ("contactType", contact_type.state()),
            ("baseSignatureParams", base_signature_params.state()),
            ("crossSectionParams", cross_section_params.state()),
            ("emissionModifierParams", emission_modifier_params.state()),
            ("deathParams", death_params.state()),
            ("scanBounds", scan_bounds.state()),
            ("roomParams", room_params.state()),
        ])?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            contact_type,
            base_signature_params,
            cross_section_params,
            emission_modifier_params,
            death_params,
            scan_bounds,
            room_params,
            authored_fields,
        })
    }
}

/// Original starmap radar room settings.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct RadarRoomParams<'source> {
    /// The original concrete type.
    pub source_type: SourcedValue<&'source str>,
    /// Whether to ignore the interior and exterior check.
    pub ignore_interior_vs_exterior_check: SourcedValue<bool>,
    /// Additional original fields.
    pub authored_fields: SourceArenaOutput<'source>,
}

impl<'source> RadarRoomParams<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let ignore_interior_vs_exterior_check =
            fields.boolean("ignoreInteriorVsExteriorCheck", notes)?;
        let authored_fields = fields.authored_members_except([(
            "ignoreInteriorVsExteriorCheck",
            ignore_interior_vs_exterior_check.state(),
        )])?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            ignore_interior_vs_exterior_check,
            authored_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct RadarSettings<'source> {
    source_type: SourcedValue<&'source str>,
    is_scannable: SourcedValue<bool>,
    radar_properties: SourcedValue<RadarProperties<'source>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> RadarSettings<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Self>, CatalogError> {
        fields.declared_class_to(
            "radarSettings",
            "StarMapPresetRadarSettings",
            notes,
            |fields, notes| {
                let is_scannable = fields.boolean("isScannable", notes)?;
                let radar_properties = RadarProperties::read_field(fields, notes)?;
                let authored_fields = fields.authored_members_except([
                    ("isScannable", is_scannable.state()),
                    ("radarProperties", radar_properties.state()),
                ])?;
                Ok(Self {
                    source_type: fields.source_type_value()?,
                    is_scannable,
                    radar_properties,
                    authored_fields,
                })
            },
        )
    }
}
