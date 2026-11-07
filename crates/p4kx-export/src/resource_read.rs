// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, StructValue};

use crate::guid_text::CatalogGuid;
use crate::record_fields;
use crate::resource_density_read;
use crate::resource_property_read;
use crate::resource_type_contract::{ResourceCargoSlot, ResourceProperty, ResourceType};
use crate::source_fields::{self, FieldError, Fields};
use crate::texts::Texts;

const RESOURCE_FIELDS: &[&str] = &[
    "displayName",
    "description",
    "densityType",
    "defaultThumbnailPath",
    "defaultThumbnailPathSVG",
    "rttThumbnailEntityClass",
    "properties",
    "refinedVersion",
    "validateDefaultCargoBox",
    "defaultCargoContainers",
];

/// A projected resource and typed source coverage diagnostics captured during projection.
pub(crate) struct ResourceRead<'source> {
    pub(crate) value: ResourceType<'source>,
    pub(crate) notes: Vec<FieldError>,
}

/// Projects a `ResourceType` from its canonical owner, without following Record References.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    texts: &Texts<'source>,
    group_guids: Vec<CatalogGuid>,
) -> Result<ResourceRead<'source>, FieldError> {
    let mut notes = Vec::new();
    let fields = Fields::new(tree.root(), "");
    fields.check(RESOURCE_FIELDS, &mut notes);
    let density = fields
        .structure("densityType")?
        .map(|source| resource_density_read::read(source, "densityType", &mut notes))
        .transpose()?;
    let default_cargo_containers = fields
        .structure("defaultCargoContainers")?
        .map(cargo_slots)
        .transpose()?;
    let properties = properties(tree.root(), &mut notes)?;
    Ok(ResourceRead {
        value: ResourceType {
            guid: CatalogGuid(tree.guid()),
            class_name: record_fields::class_name(tree),
            record_path: tree.record_path(),
            name: texts.localised(fields.locale("displayName")?),
            description: texts.localised(fields.locale("description")?),
            group_guids,
            refined_version: fields.reference("refinedVersion")?,
            density,
            default_thumbnail_path: fields.text("defaultThumbnailPath")?,
            default_thumbnail_path_svg: fields.text("defaultThumbnailPathSVG")?,
            rtt_thumbnail_entity_class: fields.reference("rttThumbnailEntityClass")?,
            validate_default_cargo_box: fields.boolean("validateDefaultCargoBox")?,
            default_cargo_containers,
            properties,
        },
        notes,
    })
}

fn cargo_slots<'source>(
    source: &StructValue<'source>,
) -> Result<Vec<ResourceCargoSlot<'source>>, FieldError> {
    let fields = Fields::new(source, "defaultCargoContainers");
    source
        .fields()
        .iter()
        .map(|field| {
            Ok(ResourceCargoSlot {
                slot: field.name(),
                entity_class: fields.reference(field.name())?,
            })
        })
        .collect()
}

fn properties<'source>(
    source: &StructValue<'source>,
    notes: &mut Vec<FieldError>,
) -> Result<Option<Vec<Option<ResourceProperty<'source>>>>, FieldError> {
    let fields = Fields::new(source, "");
    fields
        .array("properties")?
        .map(|values| {
            let mut properties = Vec::with_capacity(values.len());
            for (index, value) in values.iter().enumerate() {
                let path = format!("properties[{index}]");
                let entry = source_fields::entry(value, source.type_name(), &path)?;
                properties.push(
                    entry
                        .map(|entry| resource_property_read::read(entry, &path, notes))
                        .transpose()?,
                );
            }
            Ok(properties)
        })
        .transpose()
}
