// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_text_contract::PublicNameRef;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;
use crate::source_value_catalog::SourceRecordReference;

pub(crate) const SOURCE_TYPE: &str = "StarMapObjectType";

/// One physical starmap type and its stored navigation and display policy.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct StarmapTypeDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    name: SourcedValue<&'source str>,
    classification: SourcedValue<PublicNameRef>,
    facing_mode: SourcedValue<&'source str>,
    minimum_display_size: SourcedValue<ResourceFloat32>,
    rotation_speed: SourcedValue<ResourceFloat32>,
    #[serde(flatten)]
    flags: TypeFlags,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    marker_config: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    sub_nav_point_class: SourcedValue<Box<SourceRecordReference<'source>>>,
    geometry: SourcedValue<StarmapResourcePath<'source>>,
    material: SourcedValue<StarmapResourcePath<'source>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> StarmapTypeDefinition<'source> {
    pub(super) fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let name = fields.borrowed_string("name", notes)?;
        let classification = fields.public_name("classification", projection, notes)?;
        let facing_mode =
            fields.borrowed_enum_to("facingMode", "WorldDisplayObjectFacingMode", notes)?;
        let minimum_display_size = fields.single("minimumDisplaySize", notes)?;
        let rotation_speed = fields.single("rotationSpeed", notes)?;
        let flags = TypeFlags::read(fields, notes)?;
        let marker_config = fields.record_link_to("markerConfig", "Marker_Configuration", notes)?;
        let sub_nav_point_class =
            fields.record_link_to("subNavPointClass", "EntityClassDefinition", notes)?;
        let geometry = fields.declared_class_to(
            "geometry",
            "GlobalResourceGeometry",
            notes,
            StarmapResourcePath::read,
        )?;
        let material = fields.declared_class_to(
            "material",
            "GlobalResourceMaterial",
            notes,
            StarmapResourcePath::read,
        )?;
        let authored_fields =
            fields.authored_members_except(flags.projected().into_iter().chain([
                ("name", name.state()),
                ("classification", classification.state()),
                ("facingMode", facing_mode.state()),
                ("minimumDisplaySize", minimum_display_size.state()),
                ("rotationSpeed", rotation_speed.state()),
                ("markerConfig", marker_config.state()),
                ("subNavPointClass", sub_nav_point_class.state()),
                ("geometry", geometry.state()),
                ("material", material.state()),
            ]))?;
        Ok(Self {
            source,
            name,
            classification,
            facing_mode,
            minimum_display_size,
            rotation_speed,
            flags,
            marker_config,
            sub_nav_point_class,
            geometry,
            material,
            authored_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct TypeFlags {
    selectable: SourcedValue<bool>,
    fade_behind_parent: SourcedValue<bool>,
    on_parent_surface: SourcedValue<bool>,
    rotate_nearby_objective_markers: SourcedValue<bool>,
    spawn_nav_points: SourcedValue<bool>,
    ignore_as_routing_obstacle: SourcedValue<bool>,
    show_as_neighbor: SourcedValue<bool>,
    inner_culling: SourcedValue<bool>,
    show_in_map_select_list: SourcedValue<bool>,
    valid_quantum_travel_destination: SourcedValue<bool>,
}

impl TypeFlags {
    fn read(
        fields: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            selectable: fields.boolean("selectable", notes)?,
            fade_behind_parent: fields.boolean("fadeBehindParent", notes)?,
            on_parent_surface: fields.boolean("onParentSurface", notes)?,
            rotate_nearby_objective_markers: fields
                .boolean("rotateNearbyObjectiveMarkers", notes)?,
            spawn_nav_points: fields.boolean("spawnNavPoints", notes)?,
            ignore_as_routing_obstacle: fields.boolean("ignoreAsRoutingObstacle", notes)?,
            show_as_neighbor: fields.boolean("showAsNeighbor", notes)?,
            inner_culling: fields.boolean("innerCulling", notes)?,
            show_in_map_select_list: fields.boolean("showInMapSelectList", notes)?,
            valid_quantum_travel_destination: fields
                .boolean("validQuantumTravelDestination", notes)?,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 10] {
        [
            ("selectable", self.selectable.state()),
            ("fadeBehindParent", self.fade_behind_parent.state()),
            ("onParentSurface", self.on_parent_surface.state()),
            (
                "rotateNearbyObjectiveMarkers",
                self.rotate_nearby_objective_markers.state(),
            ),
            ("spawnNavPoints", self.spawn_nav_points.state()),
            (
                "ignoreAsRoutingObstacle",
                self.ignore_as_routing_obstacle.state(),
            ),
            ("showAsNeighbor", self.show_as_neighbor.state()),
            ("innerCulling", self.inner_culling.state()),
            ("showInMapSelectList", self.show_in_map_select_list.state()),
            (
                "validQuantumTravelDestination",
                self.valid_quantum_travel_destination.state(),
            ),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct StarmapResourcePath<'source> {
    source_type: SourcedValue<&'source str>,
    path: SourcedValue<&'source str>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> StarmapResourcePath<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let path = fields.borrowed_string("path", notes)?;
        let authored_fields = fields.authored_members_except([("path", path.state())])?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            path,
            authored_fields,
        })
    }
}
