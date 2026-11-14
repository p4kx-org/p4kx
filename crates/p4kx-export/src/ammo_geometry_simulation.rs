// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::assets::GeometryAsset;
use crate::CatalogError;
use crate::catalog_v3::authored_ammo_inputs::AmmoPhysicsBase;
use crate::catalog_v3::authored_fields::Observed;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// Original soft-body geometry asset and simulation settings.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct GeometrySimulation<'source> {
    geometry: SourcedValue<GeometryAsset<'source>>,
    simulation_params: SourcedValue<GeometrySimulationParams>,
}

impl<'source> GeometrySimulation<'source> {
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
            simulation_params: source.inline_class(
                "SimulationParams",
                &["SEntitySoftExPhysicsControllerParams"],
                notes,
                GeometrySimulationParams::read,
            )?,
        })
    }
}

/// Stored soft-body controller values without unit conversions.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct GeometrySimulationParams {
    #[serde(flatten)]
    common: AmmoPhysicsBase,
    damping: SourcedValue<ResourceFloat32>,
    stretch_stiffness: SourcedValue<ResourceFloat32>,
    compress_stiffness: SourcedValue<ResourceFloat32>,
    bending_stiffness: SourcedValue<ResourceFloat32>,
    cosserat_shear_x_stiffness: SourcedValue<ResourceFloat32>,
    cosserat_shear_y_stiffness: SourcedValue<ResourceFloat32>,
    cosserat_stretch_stiffness: SourcedValue<ResourceFloat32>,
    cosserat_bend_x_stiffness: SourcedValue<ResourceFloat32>,
    cosserat_bend_y_stiffness: SourcedValue<ResourceFloat32>,
    cosserat_twisting_stiffness: SourcedValue<ResourceFloat32>,
    attachment_influence: SourcedValue<ResourceFloat32>,
    max_displacement_influence: SourcedValue<ResourceFloat32>,
    max_stretch_attach: SourcedValue<ResourceFloat32>,
    tetra_vol_stiffness: SourcedValue<ResourceFloat32>,
    collision_gap: SourcedValue<ResourceFloat32>,
    collision_thickness_factor: SourcedValue<ResourceFloat32>,
    static_friction: SourcedValue<ResourceFloat32>,
    dynamic_friction: SourcedValue<ResourceFloat32>,
    fixed_step: SourcedValue<ResourceFloat32>,
    total_mass: SourcedValue<ResourceFloat32>,
    relative_delta_scale: SourcedValue<ResourceFloat32>,
    binding_offset: SourcedValue<ResourceFloat32>,
    enforce_length: SourcedValue<bool>,
    enforce_bending: SourcedValue<bool>,
    enforce_cosserat_stretch_shear: SourcedValue<bool>,
    enforce_cosserat_bend_twist: SourcedValue<bool>,
    enforce_attachment: SourcedValue<bool>,
    enforce_max_displacement: SourcedValue<bool>,
    enforce_tetra_vol: SourcedValue<bool>,
    enable_collisions: SourcedValue<bool>,
    enable_friction: SourcedValue<bool>,
    enable_self_collision: SourcedValue<bool>,
    normalize_particle_size: SourcedValue<bool>,
    max_displacement_restrict_to_positive_hemisphere: SourcedValue<bool>,
    iterations: SourcedValue<i32>,
    grid_dim: SourcedValue<i32>,
    lift: SourcedValue<ResourceFloat32>,
    drag: SourcedValue<ResourceFloat32>,
    wind_variance: SourcedValue<ResourceFloat32>,
    air_resistance: SourcedValue<ResourceFloat32>,
    water_resistance: SourcedValue<ResourceFloat32>,
    substep_mode: SourcedValue<String>,
    visual_binding_mode: SourcedValue<String>,
}

impl GeometrySimulationParams {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            common: AmmoPhysicsBase::read(source, notes)?,
            damping: source.single("damping", notes)?,
            stretch_stiffness: source.single("stretchStiffness", notes)?,
            compress_stiffness: source.single("compressStiffness", notes)?,
            bending_stiffness: source.single("bendingStiffness", notes)?,
            cosserat_shear_x_stiffness: source.single("cosseratShearXStiffness", notes)?,
            cosserat_shear_y_stiffness: source.single("cosseratShearYStiffness", notes)?,
            cosserat_stretch_stiffness: source.single("cosseratStretchStiffness", notes)?,
            cosserat_bend_x_stiffness: source.single("cosseratBendXStiffness", notes)?,
            cosserat_bend_y_stiffness: source.single("cosseratBendYStiffness", notes)?,
            cosserat_twisting_stiffness: source.single("cosseratTwistingStiffness", notes)?,
            attachment_influence: source.single("attachmentInfluence", notes)?,
            max_displacement_influence: source.single("maxDisplacementInfluence", notes)?,
            max_stretch_attach: source.single("maxStretchAttach", notes)?,
            tetra_vol_stiffness: source.single("tetraVolStiffness", notes)?,
            collision_gap: source.single("collisionGap", notes)?,
            collision_thickness_factor: source.single("collisionThicknessFactor", notes)?,
            static_friction: source.single("staticFriction", notes)?,
            dynamic_friction: source.single("dynamicFriction", notes)?,
            fixed_step: source.single("fixedStep", notes)?,
            total_mass: source.single("totalMass", notes)?,
            relative_delta_scale: source.single("relativeDeltaScale", notes)?,
            binding_offset: source.single("bindingOffset", notes)?,
            enforce_length: source.boolean("enforceLength", notes)?,
            enforce_bending: source.boolean("enforceBending", notes)?,
            enforce_cosserat_stretch_shear: source.boolean("enforceCosseratStrechShear", notes)?,
            enforce_cosserat_bend_twist: source.boolean("enforceCosseratBendTwist", notes)?,
            enforce_attachment: source.boolean("enforceAttachment", notes)?,
            enforce_max_displacement: source.boolean("enforceMaxDisplacement", notes)?,
            enforce_tetra_vol: source.boolean("enforceTetraVol", notes)?,
            enable_collisions: source.boolean("enableCollisions", notes)?,
            enable_friction: source.boolean("enableFriction", notes)?,
            enable_self_collision: source.boolean("enableSelfCollision", notes)?,
            normalize_particle_size: source.boolean("normalizeParticleSize", notes)?,
            max_displacement_restrict_to_positive_hemisphere: source
                .boolean("maxDisplacementRestrictToPositiveHemisphere", notes)?,
            iterations: source.int32("iterations", notes)?,
            grid_dim: source.int32("gridDim", notes)?,
            lift: source.single("lift", notes)?,
            drag: source.single("drag", notes)?,
            wind_variance: source.single("windVariance", notes)?,
            air_resistance: source.single("airResistance", notes)?,
            water_resistance: source.single("waterResistance", notes)?,
            substep_mode: source.enumeration("substepMode", notes)?,
            visual_binding_mode: source.enumeration("visualBindingMode", notes)?,
        })
    }
}
