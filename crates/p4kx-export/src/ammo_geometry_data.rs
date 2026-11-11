// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::assets::{GeometryAsset, GeometryPalette};
use super::simulation::GeometrySimulation;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::authored_fields::Observed;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;

/// Original asset paths, simulation link, paint and render settings.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct GeometryData<'source> {
    geometry: SourcedValue<GeometryAsset<'source>>,
    material: SourcedValue<GeometryAsset<'source>>,
    simulation_geometry: SourcedValue<GeometrySimulation<'source>>,
    palette: SourcedValue<GeometryPalette<'source>>,
    slot: SourcedValue<String>,
    material_attachments: SourcedValue<&'source str>,
    proxy_cdf_path: SourcedValue<&'source str>,
    modifiers_path: SourcedValue<&'source str>,
    attach_flags: SourcedValue<u32>,
    deformer_type: SourcedValue<String>,
    protos_b_shape_exclude: SourcedValue<u16>,
    vis_tp: SourcedValue<bool>,
    vis_fp: SourcedValue<bool>,
    vis_shadow: SourcedValue<bool>,
    vis_secondary_views: SourcedValue<bool>,
    wrinkle_map: SourcedValue<bool>,
    enable_decal_projection: SourcedValue<bool>,
    bbox_joint: SourcedValue<&'source str>,
    bbox_radius: SourcedValue<ResourceFloat32>,
    wear: SourcedValue<ResourceFloat32>,
    dirt: SourcedValue<ResourceFloat32>,
    interference: SourcedValue<ResourceFloat32>,
    damage: SourcedValue<ResourceFloat32>,
    render_layer: SourcedValue<String>,
    vis_area_mode: SourcedValue<String>,
    sun_shadow_mode: SourcedValue<String>,
    lod_ratio: SourcedValue<u8>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    view_dist_ratio: SourcedValue<Box<SourceRecordReference<'source>>>,
}

impl<'source> GeometryData<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            geometry: source.inline_class(
                "Geometry",
                &["GlobalResourceGeometry"],
                notes,
                GeometryAsset::read,
            )?,
            material: source.inline_class(
                "Material",
                &["GlobalResourceMaterial"],
                notes,
                GeometryAsset::read,
            )?,
            simulation_geometry: source.strong_pointer_to(
                "SimulationGeometry",
                "SSoftbodyGeometryParams",
                &["SSoftbodyGeometryParams"],
                notes,
                GeometrySimulation::read,
            )?,
            palette: source.inline_class(
                "Palette",
                &["TintPaletteRef"],
                notes,
                GeometryPalette::read,
            )?,
            slot: source.enumeration("Slot", notes)?,
            material_attachments: source.borrowed_string("MaterialAttachments", notes)?,
            proxy_cdf_path: source.borrowed_string("ProxyCDFPath", notes)?,
            modifiers_path: source.borrowed_string("ModifiersPath", notes)?,
            attach_flags: source.uint32("AttachFlags", notes)?,
            deformer_type: source.enumeration("DeformerType", notes)?,
            protos_b_shape_exclude: source.uint16("ProtosBShapeExclude", notes)?,
            vis_tp: source.boolean("VisTP", notes)?,
            vis_fp: source.boolean("VisFP", notes)?,
            vis_shadow: source.boolean("VisShadow", notes)?,
            vis_secondary_views: source.boolean("VisSecondaryViews", notes)?,
            wrinkle_map: source.boolean("WrinkleMap", notes)?,
            enable_decal_projection: source.boolean("EnableDecalProjection", notes)?,
            bbox_joint: source.borrowed_string("BBoxJoint", notes)?,
            bbox_radius: source.single("BBoxRadius", notes)?,
            wear: source.single("Wear", notes)?,
            dirt: source.single("Dirt", notes)?,
            interference: source.single("Interference", notes)?,
            damage: source.single("Damage", notes)?,
            render_layer: source.enumeration("RenderLayer", notes)?,
            vis_area_mode: source.enumeration("VisAreaMode", notes)?,
            sun_shadow_mode: source.enumeration("SunShadowMode", notes)?,
            lod_ratio: source.uint8("LodRatio", notes)?,
            view_dist_ratio: source.record_link_to(
                "viewDistRatio",
                "SViewDistanceRatioParams",
                notes,
            )?,
        })
    }
}
