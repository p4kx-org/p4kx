// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, Guid, RecordTree};

use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::CatalogFields;
use crate::catalog_read::original::{ExampleBodyReceipt, OriginalFlowFields};
use crate::catalog_v3::resource_group_catalog::{ResourceGroup, ResourceGroupDefinition};
use crate::guid_text::CatalogGuid;
use crate::record_observation::RecordObservation;
use crate::resource_group::ParentChoice;
use crate::source_fields::FieldError;
use crate::template_field_capture::TemplateFieldCapture;

/// One projected group with exact source-coverage diagnostics.
pub(crate) struct ResourceGroupRead<'source> {
    pub(crate) value: ResourceGroup<'source>,
    pub(crate) notes: Vec<FieldError>,
}

/// Projects direct scalar facts independently of raw memberships and the chosen parent.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    choice: ParentChoice<'_>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ResourceGroupRead<'source>, CatalogError> {
    let input = projection.original();
    let scope = input.record_scope(admission.record(), tree)?;
    let original = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&original, input, admission)?;
    let context = projection
        .origins()
        .record(tree.record_index())
        .with_cancel(cancel);
    let fields = CatalogFields::observed(tree.root(), "", &context, &capture)?;
    let mut notes = Vec::new();
    let definition = definition(tree, projection, &fields, &mut notes)?;
    let guid = CatalogGuid(tree.guid());
    if tree.guid() == Guid::default() {
        notes.push(
            FieldError::Malformed {
                source_type: tree.type_name().to_owned(),
                field: "$guid".to_owned(),
                expected: "a non-nil Record identity",
            }
            .with_body(ExampleBodyReceipt::technical(
                input,
                admission.subject().clone(),
                "$guid",
            )?),
        );
    }
    let parent = crate::catalog_v3::resource_group_catalog::parent(
        projection.origins(),
        tree.record_index(),
        guid,
        choice,
    )?;
    Ok(ResourceGroupRead {
        value: ResourceGroup {
            guid,
            record_path: tree.record_path(),
            record_index: tree.record_index(),
            definition,
            parent,
        },
        notes,
    })
}

fn definition<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    fields: &CatalogFields<'_, 'source, '_>,
    notes: &mut Vec<FieldError>,
) -> Result<ResourceGroupDefinition<'source>, CatalogError> {
    fields.check_observed(
        &[
            "displayName",
            "description",
            "defaultThumbnailPath",
            "defaultThumbnailPathSVG",
            "groups",
            "resources",
        ],
        notes,
    )?;
    Ok(ResourceGroupDefinition {
        source: CanonicalRecordHeader::read(tree, projection.origins())?,
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
        default_thumbnail_path: fields.text("defaultThumbnailPath", notes)?,
        default_thumbnail_path_svg: fields.text("defaultThumbnailPathSVG", notes)?,
    })
}
