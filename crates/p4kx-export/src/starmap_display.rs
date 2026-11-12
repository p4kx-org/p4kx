// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_text_contract::GameTextRef;
use crate::resource_float::{ResourceFloat32, ResourceFloat64};
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct DisplayFields<'source> {
    #[schemars(extend("x-gameText" = true))]
    description: SourcedValue<GameTextRef>,
    nav_icon: SourcedValue<&'source str>,
    size: SourcedValue<ResourceFloat64>,
    minimum_display_size: SourcedValue<ResourceFloat32>,
    use_holo_material: SourcedValue<bool>,
    star_map_geom_path: SourcedValue<&'source str>,
    star_map_material_path: SourcedValue<&'source str>,
    star_map_shape_path: SourcedValue<&'source str>,
    location_medical_image_path: SourcedValue<&'source str>,
    asset_manager_location_params: SourcedValue<StarmapAssetManager<'source>>,
}

impl<'source> DisplayFields<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            description: fields.protected_locale("description", projection, notes)?,
            nav_icon: fields.borrowed_enum_to("navIcon", "NavPointIconEnum", notes)?,
            size: fields.double("size", notes)?,
            minimum_display_size: fields.single("minimumDisplaySize", notes)?,
            use_holo_material: fields.boolean("useHoloMaterial", notes)?,
            star_map_geom_path: fields.borrowed_string("starMapGeomPath", notes)?,
            star_map_material_path: fields.borrowed_string("starMapMaterialPath", notes)?,
            star_map_shape_path: fields.borrowed_string("starMapShapePath", notes)?,
            location_medical_image_path: fields
                .borrowed_string("locationMedicalImagePath", notes)?,
            asset_manager_location_params: fields.declared_class_to(
                "assetManagerLocationParams",
                "StarMapAssetManagerLocationParams",
                notes,
                StarmapAssetManager::read,
            )?,
        })
    }

    pub(super) const fn projected(&self) -> [(&'static str, SourceValueState); 10] {
        [
            ("description", self.description.state()),
            ("navIcon", self.nav_icon.state()),
            ("size", self.size.state()),
            ("minimumDisplaySize", self.minimum_display_size.state()),
            ("useHoloMaterial", self.use_holo_material.state()),
            ("starMapGeomPath", self.star_map_geom_path.state()),
            ("starMapMaterialPath", self.star_map_material_path.state()),
            ("starMapShapePath", self.star_map_shape_path.state()),
            (
                "locationMedicalImagePath",
                self.location_medical_image_path.state(),
            ),
            (
                "assetManagerLocationParams",
                self.asset_manager_location_params.state(),
            ),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct DisplaySettings<'source> {
    source_type: SourcedValue<&'source str>,
    #[serde(flatten)]
    fields: DisplayFields<'source>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> DisplaySettings<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Self>, CatalogError> {
        fields.declared_class_to(
            "displaySettings",
            "StarMapPresetDisplaySettings",
            notes,
            |fields, notes| {
                let values = DisplayFields::read(fields, projection, notes)?;
                let authored_fields = fields.authored_members_except(values.projected())?;
                Ok(Self {
                    source_type: fields.source_type_value()?,
                    fields: values,
                    authored_fields,
                })
            },
        )
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct StarmapAssetManager<'source> {
    source_type: SourcedValue<&'source str>,
    preview_image_path: SourcedValue<&'source str>,
    preview_icon_path: SourcedValue<&'source str>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> StarmapAssetManager<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let preview_image_path = fields.borrowed_string("previewImagePath", notes)?;
        let preview_icon_path = fields.borrowed_string("previewIconPath", notes)?;
        let authored_fields = fields.authored_members_except([
            ("previewImagePath", preview_image_path.state()),
            ("previewIconPath", preview_icon_path.state()),
        ])?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            preview_image_path,
            preview_icon_path,
            authored_fields,
        })
    }
}
