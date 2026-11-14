// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::authored_nodes::AuthoredNodes;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;
use crate::{CatalogError, ExportAccountingError};

#[path = "ammo_geometry_assets.rs"]
mod assets;
#[path = "ammo_geometry_data.rs"]
mod data;
#[path = "ammo_geometry_node.rs"]
mod nodes;
#[path = "ammo_geometry_simulation.rs"]
mod simulation;

use nodes::{GeometryNode, MaterialNode};

pub(super) const SOURCE_TYPE: &str = "SGeometryResourceParams";

/// Authored geometry and material roots and their separate local node arrays.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AmmoGeometryResource<'source> {
    model_tag: SourcedValue<GeometryModelTag<'source>>,
    cache_resources: SourcedValue<bool>,
    mesh_setup: SourcedValue<GeometryMeshSetup<'source>>,
    geometry: SourcedValue<u32>,
    material: SourcedValue<u32>,
    geometry_nodes: Vec<GeometryNode<'source>>,
    material_nodes: Vec<MaterialNode<'source>>,
    root_override_paint: SourcedValue<bool>,
    inherit_model_tag_from_host: SourcedValue<bool>,
    inherit_material_from_spawn_host: SourcedValue<bool>,
}

impl<'source> AmmoGeometryResource<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut geometry_nodes = AuthoredNodes::default();
        let mut material_nodes = AuthoredNodes::default();
        let geometry =
            source.inline_class("Geometry", &[nodes::GEOMETRY_TYPE], notes, |node, _| {
                geometry_nodes.push(node)
            })?;
        let material =
            source.inline_class("Material", &[nodes::MATERIAL_TYPE], notes, |node, _| {
                material_nodes.push(node)
            })?;
        Ok(Self {
            model_tag: source.strong_pointer_to(
                "ModelTag",
                "SGeometryModelTagBase",
                &[
                    "SGeometryModelTagBase",
                    "SGeometryActorModelTag",
                    "GeometryPropModelTag",
                    "GeometryAsteroidModelTag",
                ],
                notes,
                GeometryModelTag::read,
            )?,
            cache_resources: source.boolean("cacheResources", notes)?,
            mesh_setup: source.inline_class(
                "meshsetup",
                &["SGeometryMeshsetupParams"],
                notes,
                GeometryMeshSetup::read,
            )?,
            geometry,
            material,
            geometry_nodes: geometry_nodes.read(notes, GeometryNode::read)?,
            material_nodes: material_nodes.read(notes, MaterialNode::read)?,
            root_override_paint: source.boolean("rootOverridePaint", notes)?,
            inherit_model_tag_from_host: source.boolean("inheritModelTagFromHost", notes)?,
            inherit_material_from_spawn_host: source
                .boolean("inheritMaterialFromSpawnHost", notes)?,
        })
    }
}

/// An original model-tag declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum GeometryModelTag<'source> {
    Base,
    Actor {
        #[schemars(with = "SourcedValue<CatalogRecordReference>")]
        tag: SourcedValue<Box<SourceRecordReference<'source>>>,
    },
    Prop {
        #[schemars(with = "SourcedValue<CatalogRecordReference>")]
        tag: SourcedValue<Box<SourceRecordReference<'source>>>,
    },
    Asteroid {
        #[schemars(with = "SourcedValue<CatalogRecordReference>")]
        tag: SourcedValue<Box<SourceRecordReference<'source>>>,
    },
}

impl<'source> GeometryModelTag<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            "SGeometryModelTagBase" => Self::Base,
            "SGeometryActorModelTag" => Self::Actor {
                tag: source.record_link_to("tag", "Tag", notes)?,
            },
            "GeometryPropModelTag" => Self::Prop {
                tag: source.record_link_to("tag", "Tag", notes)?,
            },
            "GeometryAsteroidModelTag" => Self::Asteroid {
                tag: source.record_link_to("tag", "Tag", notes)?,
            },
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        })
    }
}

/// Authored visibility tags selected by the mesh setup.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct GeometryMeshSetup<'source> {
    visibility_tags: SourcedValue<GeometryTagList<'source>>,
}

impl<'source> GeometryMeshSetup<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            visibility_tags: source.inline_class(
                "visibilityTags",
                &["TagList"],
                notes,
                GeometryTagList::read,
            )?,
        })
    }
}

/// The ordered original tag Reference slots.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct GeometryTagList<'source> {
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    tags: SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>,
}

impl<'source> GeometryTagList<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            tags: source.record_link_array_to("tags", "Tag", notes)?,
        })
    }
}
