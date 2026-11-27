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

/// Original particle dimensions, movement controls and collision inputs.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AmmoParticlePhysics<'source> {
    pierceability: SourcedValue<u32>,
    radius: SourcedValue<ResourceFloat32>,
    thickness: SourcedValue<ResourceFloat32>,
    acc_thrust: SourcedValue<ResourceFloat32>,
    air_resistance: SourcedValue<ResourceFloat32>,
    length: SourcedValue<ResourceFloat32>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    resource_type: SourcedValue<Box<SourceRecordReference<'source>>>,
    ray_collision: SourcedValue<bool>,
    traceable: SourcedValue<bool>,
    single_contact: SourcedValue<bool>,
    constant_orientation: SourcedValue<bool>,
    no_roll: SourcedValue<bool>,
    no_spin: SourcedValue<bool>,
    no_path_alignment: SourcedValue<bool>,
    no_self_collision: SourcedValue<bool>,
    no_impulse: SourcedValue<bool>,
    decouple_heading: SourcedValue<bool>,
    disable_gravity: SourcedValue<bool>,
    ai_navigation_type: SourcedValue<String>,
    surface_id_name: SourcedValue<&'source str>,
}

impl<'source> AmmoParticlePhysics<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            pierceability: source.uint32("pierceability", notes)?,
            radius: source.single("radius", notes)?,
            thickness: source.single("thickness", notes)?,
            acc_thrust: source.single("accThrust", notes)?,
            air_resistance: source.single("airResistance", notes)?,
            length: source.single("length", notes)?,
            resource_type: source.record_link("resourceType", notes)?,
            ray_collision: source.boolean("rayCollision", notes)?,
            traceable: source.boolean("traceable", notes)?,
            single_contact: source.boolean("singleContact", notes)?,
            constant_orientation: source.boolean("constantOrientation", notes)?,
            no_roll: source.boolean("noRoll", notes)?,
            no_spin: source.boolean("noSpin", notes)?,
            no_path_alignment: source.boolean("noPathAlignment", notes)?,
            no_self_collision: source.boolean("noSelfCollision", notes)?,
            no_impulse: source.boolean("noImpulse", notes)?,
            decouple_heading: source.boolean("decoupleHeading", notes)?,
            disable_gravity: source.boolean("disableGravity", notes)?,
            ai_navigation_type: source.enumeration("aiNavigationType", notes)?,
            surface_id_name: source.borrowed_string("surfaceIdName", notes)?,
        })
    }
}
