// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

pub(super) const RADAR: &str = "radarOptionParams";
const RADAR_TYPE: &str = "RadarOptionParams";

/// Authored radar options from a physically identified game-mode Record.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct RadarOptionsConfiguration<'source> {
    source: CanonicalRecordHeader<'source>,
    game_mode_id: SourcedValue<&'source str>,
    params: SourcedValue<RadarModeParameters<'source>>,
}

impl<'source> RadarOptionsConfiguration<'source> {
    pub(super) const fn new(
        source: CanonicalRecordHeader<'source>,
        game_mode_id: SourcedValue<&'source str>,
        params: SourcedValue<RadarModeParameters<'source>>,
    ) -> Self {
        Self {
            source,
            game_mode_id,
            params,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct RadarModeParameters<'source> {
    source_type: SourcedValue<&'source str>,
    radar_option_params: SourcedValue<RadarOptionParameters<'source>>,
    raw_radar_option_params: Option<SourcedValue<SourceArenaOutput<'source>>>,
}

impl<'source> RadarModeParameters<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        source_type: SourcedValue<&'source str>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let radar_option_params =
            source.declared_class_to(RADAR, RADAR_TYPE, notes, RadarOptionParameters::read)?;
        let raw_radar_option_params = matches!(
            radar_option_params.state(),
            SourceValueState::Unsupported | SourceValueState::Invalid
        )
        .then(|| source.authored_member(RADAR, notes))
        .transpose()?;
        Ok(Self {
            source_type,
            radar_option_params,
            raw_radar_option_params,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct RadarOptionParameters<'source> {
    source_type: SourcedValue<&'source str>,
    #[serde(rename = "useFPSTagging")]
    use_fps_tagging: SourcedValue<bool>,
    #[serde(rename = "useFPSScanMode")]
    use_fps_scan_mode: SourcedValue<bool>,
    enable_scan_mode: SourcedValue<bool>,
    use_experimental_features: SourcedValue<bool>,
    allow_delta_signature: SourcedValue<bool>,
    #[serde(rename = "allowEmissionsHUD")]
    allow_emissions_hud: SourcedValue<bool>,
    #[serde(rename = "allowFPSPingWave")]
    allow_fps_ping_wave: SourcedValue<bool>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> RadarOptionParameters<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let use_fps_tagging = source.boolean("useFPSTagging", notes)?;
        let use_fps_scan_mode = source.boolean("useFPSScanMode", notes)?;
        let enable_scan_mode = source.boolean("enableScanMode", notes)?;
        let use_experimental_features = source.boolean("useExperimentalFeatures", notes)?;
        let allow_delta_signature = source.boolean("allowDeltaSignature", notes)?;
        let allow_emissions_hud = source.boolean("allowEmissionsHUD", notes)?;
        let allow_fps_ping_wave = source.boolean("allowFPSPingWave", notes)?;
        let additional_fields = source.authored_members_except([
            ("useFPSTagging", use_fps_tagging.state()),
            ("useFPSScanMode", use_fps_scan_mode.state()),
            ("enableScanMode", enable_scan_mode.state()),
            ("useExperimentalFeatures", use_experimental_features.state()),
            ("allowDeltaSignature", allow_delta_signature.state()),
            ("allowEmissionsHUD", allow_emissions_hud.state()),
            ("allowFPSPingWave", allow_fps_ping_wave.state()),
        ])?;
        Ok(Self {
            source_type: source.source_type_value()?,
            use_fps_tagging,
            use_fps_scan_mode,
            enable_scan_mode,
            use_experimental_features,
            allow_delta_signature,
            allow_emissions_hud,
            allow_fps_ping_wave,
            additional_fields,
        })
    }
}
