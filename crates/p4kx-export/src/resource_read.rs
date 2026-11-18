// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree, SourceValue, SourcedValue, StructValue};

use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::NumberCapture;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_read::{CatalogFields, CatalogRecordContext};
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_text_contract::PublicNameRef;
use crate::guid_text::CatalogGuid;
use crate::record_observation::RecordObservation;
use crate::resource_density_read;
use crate::resource_group::ResourceMemberships;
use crate::resource_memberships;
use crate::resource_property_contract::{ResourceCargoSlot, ResourceProperty};
use crate::resource_property_read;
use crate::resource_type_contract::{ResourceDefinition, ResourceType};
use crate::resource_value_read;
use crate::source_fields::{FieldCapture, FieldError, FieldViolation};
use crate::template_field_capture::TemplateFieldCapture;

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
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ResourceRead<'source>, CatalogError> {
    read_with_memberships(tree, projection, admission, cancel, || Ok(memberships))
}

/// Projects the complete definition and requests reverse memberships only at their canonical field.
pub(crate) fn read_with_memberships<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
    memberships: impl FnOnce() -> Result<ResourceMemberships, CatalogError>,
) -> Result<ResourceRead<'source>, CatalogError> {
    let mut notes = Vec::new();
    let numbers = NumberCapture::default();
    let data = with_fields(
        tree,
        projection,
        admission,
        cancel,
        Some(&numbers),
        |fields, capture| definition(tree, projection, memberships, &mut notes, fields, capture),
    )?;
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

pub(crate) struct NameAndRefinement {
    pub(crate) name: SourcedValue<PublicNameRef>,
    pub(crate) refined_version: SourcedValue<CatalogRecordReference>,
}

pub(crate) fn read_name_and_refinement<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(NameAndRefinement, Vec<FieldError>), CatalogError> {
    let mut notes = Vec::new();
    let value = with_fields(tree, projection, admission, cancel, None, |fields, _| {
        name_and_refinement(tree, projection, fields, &mut notes)
    })?;
    Ok((value, notes))
}

fn with_fields<'source, T>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
    numbers: Option<&NumberCapture>,
    read: impl FnOnce(
        &CatalogFields<'_, 'source, '_>,
        &dyn FieldCapture<'source>,
    ) -> Result<T, CatalogError>,
) -> Result<T, CatalogError> {
    let input = projection.original();
    let scope = input.record_scope(admission.record(), tree)?;
    let original = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&original, input, admission)?;
    let context = projection.origins().record(tree.record_index());
    let context = numbers.map_or(context, |capture| context.with_numbers(capture));
    let fields = CatalogFields::observed(tree.root(), "", &context, &capture)?;
    read(&fields, &capture)
}

fn name_and_refinement<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    fields: &CatalogFields<'_, 'source, '_>,
    notes: &mut Vec<FieldError>,
) -> Result<NameAndRefinement, CatalogError> {
    Ok(NameAndRefinement {
        name: projection.name(
            fields.locale("displayName", notes)?,
            tree.record_index(),
            "displayName",
        )?,
        refined_version: resource_value_read::reference(fields, "refinedVersion", notes)?,
    })
}

fn definition<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    memberships: impl FnOnce() -> Result<ResourceMemberships, CatalogError>,
    notes: &mut Vec<FieldError>,
    fields: &CatalogFields<'_, 'source, '_>,
    capture: &dyn FieldCapture<'source>,
) -> Result<ResourceDefinition<'source>, CatalogError> {
    let origins = projection.origins();
    let context = fields.context();
    fields.check_observed(RESOURCE_FIELDS, notes)?;
    let density = fields.structure("densityType", notes)?.try_map(|value| {
        resource_density_read::read(value, "densityType", context, notes, capture)
    })?;
    let containers = fields
        .structure("defaultCargoContainers", notes)?
        .try_map(|value| cargo_slots(value, context, notes, capture))?;
    let properties = properties(fields, context, notes, capture)?;
    let NameAndRefinement {
        name,
        refined_version,
    } = name_and_refinement(tree, projection, fields, notes)?;
    Ok(ResourceDefinition {
        source: CanonicalRecordHeader::read(tree, origins)?,
        name,
        description: projection.description(
            fields.locale("description", notes)?,
            tree.record_index(),
            "description",
        )?,
        group_guids: resource_memberships::values(memberships()?, origins, tree.record_index())?,
        refined_version,
        density,
        default_thumbnail_path: fields.text("defaultThumbnailPath", notes)?,
        default_thumbnail_path_svg: fields.text("defaultThumbnailPathSVG", notes)?,
        rtt_thumbnail_entity_class: resource_value_read::reference(
            fields,
            "rttThumbnailEntityClass",
            notes,
        )?,
        validate_default_cargo_box: fields.boolean("validateDefaultCargoBox", notes)?,
        default_cargo_containers: containers,
        properties,
    })
}

fn properties<'source>(
    fields: &CatalogFields<'_, 'source, '_>,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
    capture: &dyn FieldCapture<'source>,
) -> Result<SourcedValue<Vec<SourcedValue<ResourceProperty<'source>>>>, CatalogError> {
    fields.array("properties", notes)?.try_map(|values| {
        values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let path = format!("properties[{index}]");
                fields
                    .structure_element(&path, value, notes)?
                    .try_map(|entry| {
                        resource_property_read::read(entry, &path, context, notes, capture)
                    })
            })
            .collect::<Result<_, CatalogError>>()
    })
}

fn cargo_slots<'source>(
    source: &StructValue<'source>,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
    capture: &dyn FieldCapture<'source>,
) -> Result<Vec<SourcedValue<ResourceCargoSlot<'source>>>, CatalogError> {
    source
        .fields()
        .iter()
        .zip(0_u64..)
        .map(|(field, ordinal)| {
            let context = context.cargo_slot(ordinal);
            let path = format!("defaultCargoContainers.{}", field.name());
            let value = ResourceCargoSlot {
                slot: context.selector(
                    &format!("{path}.$fieldName"),
                    SourceValue::Present(field.name()),
                )?,
                entity_class: crate::catalog_read::reference_with_failure(
                    &context,
                    Some(field.value()),
                    &path,
                    notes,
                    |value| {
                        let violation = FieldViolation::invalid(
                            source,
                            "defaultCargoContainers",
                            field.name(),
                            value,
                            "a nullable Record Reference",
                        );
                        let body = capture.field_failure(&violation)?;
                        Ok(violation.into_error().with_body(body))
                    },
                )?,
            };
            context.value(&path, SourceValue::Present(value))
        })
        .collect::<Result<_, CatalogError>>()
}
