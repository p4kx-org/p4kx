// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::starmap_display::DisplaySettings;
use super::starmap_navigation::{
    LOCATION_TYPE, StarmapLocation, StarmapQuantumTravel, TRAVEL_TYPE,
};
use super::starmap_radar::RadarSettings;
use super::starmap_type_definition;
use super::starmap_visibility::VisibilitySettings;
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;
use crate::source_value_catalog::SourceRecordReference;

pub(crate) const SOURCE_TYPE: &str = "StarMapPreset";

/// One physical starmap preset with independently sourced owned settings.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct StarmapPresetDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    r#type: SourcedValue<Box<SourceRecordReference<'source>>>,
    visibility_settings: SourcedValue<VisibilitySettings<'source>>,
    display_settings: SourcedValue<DisplaySettings<'source>>,
    quantum_travel_data: SourcedValue<StarmapQuantumTravel<'source>>,
    location_params: SourcedValue<StarmapLocation<'source>>,
    radar_settings: SourcedValue<RadarSettings<'source>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> StarmapPresetDefinition<'source> {
    pub(super) fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let r#type = fields.record_link_to("type", starmap_type_definition::SOURCE_TYPE, notes)?;
        let visibility_settings = VisibilitySettings::read(fields, notes)?;
        let display_settings = DisplaySettings::read(fields, projection, notes)?;
        let quantum_travel_data = fields.strong_pointer_family_to(
            "quantumTravelData",
            TRAVEL_TYPE,
            notes,
            StarmapQuantumTravel::read,
        )?;
        let location_params = fields.declared_class_to(
            "locationParams",
            LOCATION_TYPE,
            notes,
            StarmapLocation::read,
        )?;
        let radar_settings = RadarSettings::read(fields, notes)?;
        let authored_fields = fields.authored_members_except([
            ("type", r#type.state()),
            ("visibilitySettings", visibility_settings.state()),
            ("displaySettings", display_settings.state()),
            ("quantumTravelData", quantum_travel_data.state()),
            ("locationParams", location_params.state()),
            ("radarSettings", radar_settings.state()),
        ])?;
        Ok(Self {
            source,
            r#type,
            visibility_settings,
            display_settings,
            quantum_travel_data,
            location_params,
            radar_settings,
            authored_fields,
        })
    }
}
