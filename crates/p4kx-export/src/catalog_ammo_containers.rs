// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::catalog_table::CatalogTable;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceText;

pub(super) use crate::item_stats::AMMO_CONTAINER as SOURCE_TYPE;
const FRAGMENT: &str = "SFragmentParams";
const HIDE_ATTACHMENT: &str = "SHideAmmoAttachmentParams";
const ATTACHABLE_ENTITY: &str = "SAttachableEntityParams";

/// The authored configuration of one physical Item ammunition container.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AmmoContainer<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    source_type: SourcedValue<&'source str>,
    initial_ammo_count: SourcedValue<i32>,
    max_ammo_count: SourcedValue<i32>,
    max_restock_count: SourcedValue<u32>,
    low_ammo_warning_percentage: SourcedValue<ResourceFloat32>,
    #[serde(rename = "ammoContainerType")]
    container_type: SourcedValue<String>,
    #[serde(rename = "despawnEmptyAmmoContainer")]
    despawn_when_empty: SourcedValue<bool>,
    allow_ammo_repool: SourcedValue<bool>,
    empty_geometry_tag: SourcedValue<SourceText<'source>>,
    ammo_count_fragment: SourcedValue<AmmoFragment<'source>>,
    ammo_count_animation_blend_time: SourcedValue<ResourceFloat32>,
    hide_attachments: SourcedValue<Vec<SourcedValue<HideAttachment<'source>>>>,
    attachable_entities: SourcedValue<Vec<SourcedValue<AttachableEntity<'source>>>>,
}

impl<'source> AmmoContainer<'source> {
    pub(super) fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            owner,
            source_type: source.source_type_value()?,
            initial_ammo_count: source.int32("initialAmmoCount", notes)?,
            max_ammo_count: source.int32("maxAmmoCount", notes)?,
            max_restock_count: source.uint32("maxRestockCount", notes)?,
            low_ammo_warning_percentage: source.single("lowAmmoWarningPercentage", notes)?,
            container_type: source.enumeration("ammoContainerType", notes)?,
            despawn_when_empty: source.boolean("despawnEmptyAmmoContainer", notes)?,
            allow_ammo_repool: source.boolean("allowAmmoRepool", notes)?,
            empty_geometry_tag: text(source, SOURCE_TYPE, "emptyGeometryTag", notes)?,
            ammo_count_fragment: source.inline_class(
                "ammoCountFragment",
                &[FRAGMENT],
                notes,
                AmmoFragment::read,
            )?,
            ammo_count_animation_blend_time: source.single("ammoCountAnimationBlendTime", notes)?,
            hide_attachments: source.class_array(
                "hideAttachments",
                &[HIDE_ATTACHMENT],
                notes,
                HideAttachment::read,
            )?,
            attachable_entities: source.class_array(
                "attachableEntities",
                &[ATTACHABLE_ENTITY],
                notes,
                AttachableEntity::read,
            )?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct AmmoFragment<'source> {
    fragment: SourcedValue<SourceText<'source>>,
    force_weapon_controller: SourcedValue<bool>,
}

impl<'source> AmmoFragment<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            fragment: text(source, FRAGMENT, "fragment", notes)?,
            force_weapon_controller: source.boolean("forceWeaponController", notes)?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct HideAttachment<'source> {
    attachment_name: SourcedValue<SourceText<'source>>,
    ammo_threshold: SourcedValue<i32>,
    visible: SourcedValue<bool>,
}

impl<'source> HideAttachment<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            attachment_name: text(source, HIDE_ATTACHMENT, "attachmentName", notes)?,
            ammo_threshold: source.int32("ammoThreshold", notes)?,
            visible: source.boolean("visible", notes)?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct AttachableEntity<'source> {
    source_port: SourcedValue<SourceText<'source>>,
    target_port: SourcedValue<SourceText<'source>>,
    entity_class_name: SourcedValue<SourceText<'source>>,
}

impl<'source> AttachableEntity<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            source_port: text(source, ATTACHABLE_ENTITY, "sourcePort", notes)?,
            target_port: text(source, ATTACHABLE_ENTITY, "targetPort", notes)?,
            entity_class_name: text(source, ATTACHABLE_ENTITY, "entityClassName", notes)?,
        })
    }
}

fn text<'source>(
    source: Observed<'_, '_, '_, 'source>,
    owner: &str,
    field: &str,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<SourceText<'source>>, CatalogError> {
    Ok(source
        .borrowed_string(field, notes)?
        .map(|text| SourceText::read(owner, field, text)))
}

pub(super) fn table<'source>(
    forge: &DataForge<'_>,
    mut rows: Vec<AmmoContainer<'source>>,
) -> CatalogTable<AmmoContainer<'source>> {
    rows.sort_by_key(|row| row.owner.key());
    CatalogTable::new(forge, &[SOURCE_TYPE], rows)
}
