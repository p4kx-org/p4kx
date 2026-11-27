// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, Guid, RecordTree, TAG_DATABASE_TYPE, TagPathEdgeKind};

use super::contract::{TagDefinition, TagRoots};
use super::path_capture::NameEvidence;
use super::{fields, header};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::CatalogFields;
use crate::catalog_read::original::{ExampleBodyReceipt, OriginalFlowFields, TagRelationReceipts};
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::template_field_capture::TemplateFieldCapture;

pub(crate) enum Declaration<'source> {
    Definition(TagDefinition<'source>),
    Roots(TagRoots<'source>),
}

pub(crate) struct Read<'source> {
    pub(crate) row: Declaration<'source>,
    pub(crate) notes: Vec<FieldError>,
    pub(super) path_name: Option<NameEvidence>,
}

pub(crate) fn read_declaration<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    relations: Option<&mut TagRelationReceipts>,
    cancel: Option<&CancelToken>,
) -> Result<Read<'source>, CatalogError> {
    let result = read_parts(tree, input, admission, relations, cancel, true)?;
    Ok(Read {
        row: result
            .row
            .ok_or(crate::ExportAccountingError::ConflictingTagTrace)?,
        notes: result.notes,
        path_name: result.path_name,
    })
}

pub(super) fn read_path_name<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    relations: &mut TagRelationReceipts,
    cancel: Option<&CancelToken>,
) -> Result<Option<NameEvidence>, CatalogError> {
    Ok(read_parts(tree, input, admission, Some(relations), cancel, false)?.path_name)
}

struct Parts<'source> {
    row: Option<Declaration<'source>>,
    notes: Vec<FieldError>,
    path_name: Option<NameEvidence>,
}

fn read_parts<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    relations: Option<&mut TagRelationReceipts>,
    cancel: Option<&CancelToken>,
    project_rows: bool,
) -> Result<Parts<'source>, CatalogError> {
    let context = input.origins().record(tree.record_index());
    let scope = input.record_scope(admission.record(), tree)?;
    let original = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&original, input, admission)?;
    let fields = CatalogFields::observed(tree.root(), "", &context, &capture)?;
    let is_database = tree.type_name() == TAG_DATABASE_TYPE;
    if let Some(relations) = relations {
        let edge = if is_database {
            TagPathEdgeKind::Root
        } else {
            TagPathEdgeKind::Child
        };
        relations.capture(edge, tree, &original, capture.example_body())?;
    }
    if !project_rows {
        let mut notes = Vec::new();
        let path_name = if is_database {
            None
        } else {
            Some(fields::name(&fields, &original, &mut notes)?.1)
        };
        return Ok(Parts {
            row: None,
            notes,
            path_name,
        });
    }
    project(
        tree,
        input,
        admission,
        &fields,
        &original,
        is_database,
        cancel,
    )
}

fn project<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    fields: &CatalogFields<'_, 'source, '_>,
    original: &OriginalFlowFields<'_, '_, 'source>,
    is_database: bool,
    cancel: Option<&CancelToken>,
) -> Result<Parts<'source>, CatalogError> {
    let mut notes = Vec::new();
    let (row, path_name) = if is_database {
        fields.check_observed(&["tags"], &mut notes)?;
        let row = TagRoots {
            source: header::read(tree, input.origins())?,
            tags: fields::references(fields, "tags", &mut notes, cancel)?,
        };
        (Declaration::Roots(row), None)
    } else {
        fields.check_observed(&["tagName", "legacyGUID", "children"], &mut notes)?;
        let (tag_name, name) = fields::name(fields, original, &mut notes)?;
        let legacy_guid = fields::legacy_guid(fields, &mut notes)?;
        let row = TagDefinition {
            source: header::read(tree, input.origins())?,
            tag_name,
            legacy_guid,
            children: fields::references(fields, "children", &mut notes, cancel)?,
        };
        (Declaration::Definition(row), Some(name))
    };
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
    Ok(Parts {
        row: Some(row),
        notes,
        path_name,
    })
}
