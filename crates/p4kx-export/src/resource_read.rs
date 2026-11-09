// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, SourceValue, SourcedValue, StructValue};

use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::NumberCapture;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::{CatalogFields, CatalogRecordContext};
use crate::guid_text::CatalogGuid;
use crate::resource_density_read;
use crate::resource_group::ResourceMemberships;
use crate::resource_memberships;
use crate::resource_property_contract::{ResourceCargoSlot, ResourceProperty};
use crate::resource_property_read;
use crate::resource_type_contract::{ResourceDefinition, ResourceType};
use crate::resource_value_read;
use crate::source_fields::FieldError;

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

pub(crate) struct ResourceRead<'source> {
    pub(crate) value: ResourceType<'source>,
    pub(crate) notes: Vec<FieldError>,
}

/// Projects every supported resource value from one canonical tree without following targets.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    memberships: ResourceMemberships,
) -> Result<ResourceRead<'source>, CatalogError> {
    let mut notes = Vec::new();
    let numbers = NumberCapture::default();
    let data = definition(tree, projection, memberships, &numbers, &mut notes)?;
    Ok(ResourceRead {
        value: ResourceType {
            guid: CatalogGuid(tree.guid()),
            record_path: tree.record_path(),
            data,
            numbers: numbers.finish(),
        },
        notes,
    })
}

fn definition<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    memberships: ResourceMemberships,
    numbers: &NumberCapture,
    notes: &mut Vec<FieldError>,
) -> Result<ResourceDefinition<'source>, CatalogError> {
    let origins = projection.origins();
    let context = origins.record(tree.record_index()).with_numbers(numbers);
    let fields = CatalogFields::new(tree.root(), "", &context);
    fields.check(RESOURCE_FIELDS, notes);
    let density = fields
        .structure("densityType", notes)?
        .try_map(|value| resource_density_read::read(value, "densityType", &context, notes))?;
    let containers = fields
        .structure("defaultCargoContainers", notes)?
        .try_map(|value| cargo_slots(value, &context, notes))?;
    let properties = properties(tree, &fields, &context, notes)?;
    Ok(ResourceDefinition {
        source: CanonicalRecordHeader::read(tree, origins)?,
        name: projection.name(
            fields.locale("displayName", notes)?,
            tree.record_index(),
            "displayName",
        )?,
        description: projection.description(
            fields.locale("description", notes)?,
            tree.record_index(),
            "description",
        )?,
        group_guids: resource_memberships::values(memberships, origins, tree.record_index())?,
        refined_version: resource_value_read::reference(&fields, "refinedVersion", notes)?,
        density,
        default_thumbnail_path: fields.text("defaultThumbnailPath", notes)?,
        default_thumbnail_path_svg: fields.text("defaultThumbnailPathSVG", notes)?,
        rtt_thumbnail_entity_class: resource_value_read::reference(
            &fields,
            "rttThumbnailEntityClass",
            notes,
        )?,
        validate_default_cargo_box: fields.boolean("validateDefaultCargoBox", notes)?,
        default_cargo_containers: containers,
        properties,
    })
}

fn properties<'source>(
    tree: &RecordTree<'source>,
    fields: &CatalogFields<'_, 'source, '_>,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<Vec<SourcedValue<ResourceProperty<'source>>>>, CatalogError> {
    fields.array("properties", notes)?.try_map(|values| {
        values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let path = format!("properties[{index}]");
                context
                    .structure_element(value, tree.type_name(), &path, notes)?
                    .try_map(|entry| resource_property_read::read(entry, &path, context, notes))
            })
            .collect::<Result<_, CatalogError>>()
    })
}

fn cargo_slots<'source>(
    source: &StructValue<'source>,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
) -> Result<Vec<SourcedValue<ResourceCargoSlot<'source>>>, CatalogError> {
    let fields = CatalogFields::new(source, "defaultCargoContainers", context);
    source
        .fields()
        .iter()
        .map(|field| {
            let path = format!("defaultCargoContainers.{}", field.name());
            let value = ResourceCargoSlot {
                slot: context.selector(
                    &format!("{path}.$fieldName"),
                    SourceValue::Present(field.name()),
                )?,
                entity_class: resource_value_read::reference(&fields, field.name(), notes)?,
            };
            context.value(&path, SourceValue::Present(value))
        })
        .collect::<Result<_, CatalogError>>()
}
