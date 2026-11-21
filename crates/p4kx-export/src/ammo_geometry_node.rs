// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::assets::{GeometryAsset, GeometryPalette};
use super::data::GeometryData;
use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::authored_nodes::AuthoredNodes;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

pub(super) const GEOMETRY_TYPE: &str = "SGeometryNodeParams";
pub(super) const MATERIAL_TYPE: &str = "SMaterialNodeParams";

/// A geometry node with local child indices in `geometryNodes`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct GeometryNode<'source> {
    tags: SourcedValue<&'source str>,
    data: SourcedValue<GeometryData<'source>>,
    scale_multiplier: SourcedValue<ResourceFloat32>,
    sub_geometry: SourcedValue<Vec<SourcedValue<u32>>>,
}

impl<'source> GeometryNode<'source> {
    pub(super) fn read<'reader, 'scope, 'read>(
        source: Observed<'reader, 'scope, 'read, 'source>,
        pending: &mut AuthoredNodes<'reader, 'scope, 'read, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            tags: source.borrowed_string("Tags", notes)?,
            data: source.inline_class(
                "Geometry",
                &["SGeometryDataParams"],
                notes,
                GeometryData::read,
            )?,
            scale_multiplier: source.single("ScaleMultiplier", notes)?,
            sub_geometry: source.class_array_to(
                "SubGeometry",
                GEOMETRY_TYPE,
                notes,
                |node, _| pending.push(node),
            )?,
        })
    }
}

/// A material node with local variant indices in `materialNodes`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MaterialNode<'source> {
    tags: SourcedValue<&'source str>,
    material: SourcedValue<GeometryAsset<'source>>,
    palette: SourcedValue<GeometryPalette<'source>>,
    material_variants: SourcedValue<Vec<SourcedValue<u32>>>,
}

impl<'source> MaterialNode<'source> {
    pub(super) fn read<'reader, 'scope, 'read>(
        source: Observed<'reader, 'scope, 'read, 'source>,
        pending: &mut AuthoredNodes<'reader, 'scope, 'read, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            tags: source.borrowed_string("Tags", notes)?,
            material: source.inline_class(
                "Material",
                &["GlobalResourceMaterial"],
                notes,
                GeometryAsset::read,
            )?,
            palette: source.inline_class(
                "Palette",
                &["TintPaletteRef"],
                notes,
                GeometryPalette::read,
            )?,
            material_variants: source.class_array_to(
                "materialVariants",
                MATERIAL_TYPE,
                notes,
                |node, _| pending.push(node),
            )?,
        })
    }
}
