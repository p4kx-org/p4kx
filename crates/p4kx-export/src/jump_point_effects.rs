// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct JumpPointEffects<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    tunnel_effect_tinting: SourcedValue<TunnelTinting<'source>>,
    cube_map_params: SourcedValue<CubeMapParameters<'source>>,
}

impl<'source> JumpPointEffects<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let tunnel_effect_tinting = fields.declared_class_to(
            "tunnelEffectTinting",
            "JumpTunnelEffectTintingParams",
            notes,
            TunnelTinting::read,
        )?;
        let cube_map_params = fields.declared_class_to(
            "cubeMapParams",
            "JumpTunnelCubeMapParams",
            notes,
            CubeMapParameters::read,
        )?;
        let source = UnitSource::read(
            fields,
            [
                ("tunnelEffectTinting", tunnel_effect_tinting.state()),
                ("cubeMapParams", cube_map_params.state()),
            ],
        )?;
        Ok(Self {
            source,
            tunnel_effect_tinting,
            cube_map_params,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct TunnelTinting<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    start_glow: SourcedValue<ResourceFloat32>,
    end_glow: SourcedValue<ResourceFloat32>,
    distance_before_mid_point: SourcedValue<ResourceFloat32>,
    distance_after_mid_point: SourcedValue<ResourceFloat32>,
    start_color: SourcedValue<SourceArenaOutput<'source>>,
    mid_color: SourcedValue<SourceArenaOutput<'source>>,
    end_color: SourcedValue<SourceArenaOutput<'source>>,
}

impl<'source> TunnelTinting<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let start_glow = fields.single("startGlow", notes)?;
        let end_glow = fields.single("endGlow", notes)?;
        let distance_before_mid_point = fields.single("distanceBeforeMidPoint", notes)?;
        let distance_after_mid_point = fields.single("distanceAfterMidPoint", notes)?;
        let start_color = fields.authored_member("startColor", notes)?;
        let mid_color = fields.authored_member("midColor", notes)?;
        let end_color = fields.authored_member("endColor", notes)?;
        let source = UnitSource::read(
            fields,
            [
                ("startGlow", start_glow.state()),
                ("endGlow", end_glow.state()),
                ("distanceBeforeMidPoint", distance_before_mid_point.state()),
                ("distanceAfterMidPoint", distance_after_mid_point.state()),
                ("startColor", start_color.state()),
                ("midColor", mid_color.state()),
                ("endColor", end_color.state()),
            ],
        )?;
        Ok(Self {
            source,
            start_glow,
            end_glow,
            distance_before_mid_point,
            distance_after_mid_point,
            start_color,
            mid_color,
            end_color,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct CubeMapParameters<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    entry_spec_probe: SourcedValue<Probe<'source>>,
    entry_diff_probe: SourcedValue<Probe<'source>>,
    exit_spec_probe: SourcedValue<Probe<'source>>,
    exit_diff_probe: SourcedValue<Probe<'source>>,
}

impl<'source> CubeMapParameters<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut read =
            |field| fields.declared_class_to(field, "GlobalResourceTextureDDS", notes, Probe::read);
        let entry_spec_probe = read("entrySpecProbe")?;
        let entry_diff_probe = read("entryDiffProbe")?;
        let exit_spec_probe = read("exitSpecProbe")?;
        let exit_diff_probe = read("exitDiffProbe")?;
        let source = UnitSource::read(
            fields,
            [
                ("entrySpecProbe", entry_spec_probe.state()),
                ("entryDiffProbe", entry_diff_probe.state()),
                ("exitSpecProbe", exit_spec_probe.state()),
                ("exitDiffProbe", exit_diff_probe.state()),
            ],
        )?;
        Ok(Self {
            source,
            entry_spec_probe,
            entry_diff_probe,
            exit_spec_probe,
            exit_diff_probe,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct Probe<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    path: SourcedValue<&'source str>,
}

impl<'source> Probe<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let path = fields.borrowed_string("path", notes)?;
        let source = UnitSource::read(fields, [("path", path.state())])?;
        Ok(Self { source, path })
    }
}
