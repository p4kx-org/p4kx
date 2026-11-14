// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::{
    CancelToken, DataForge, DataForgeError, Record, RecordTree, TagBuildEvent, TagPathEdgeKind,
    TagTree,
};

use super::contract::{TagDefinition, TagRoots, TagRows};
use super::path_capture::{ChosenPath, NameEvidence, PathCapture};
use super::{diagnostics, fields, header};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::CatalogFields;
use crate::catalog_read::original::TagRelationReceipts;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::guid_text::CatalogGuid;
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::{CatalogError, CatalogReadFailure, Kind};

#[path = "captured_tags.rs"]
mod captured;

pub(crate) use captured::{
    CapturedTagTree, CapturedTags, ContextualTagPath, OriginalTagPath, TagDatum, TagNameSelector,
    TagPathLookup, TagPathMeaning,
};

pub(crate) struct TagRead<'context, 'source> {
    pub(crate) rows: TagRows<'source>,
    pub(crate) context: CapturedTags<'context, 'source>,
    pub(crate) dependencies: Option<TagDependencies<'source>>,
    pub(crate) ledger: ExportLedger,
    pub(crate) failures: Vec<CatalogReadFailure<'source>>,
}

/// The original dependency observations from the same Tag build.
pub(crate) struct TagDependencies<'source> {
    pub(crate) tree: CapturedTagTree,
    pub(crate) failures: Vec<CatalogReadFailure<'source>>,
    pub(crate) ledger: ExportLedger,
}

struct Capture<'context, 'source> {
    input: &'context OriginalCatalogInput<'context, 'source>,
    cancel: Option<&'context CancelToken>,
    rows: TagRows<'source>,
    paths: PathCapture,
    relations: TagRelationReceipts,
    names: BTreeMap<u32, NameEvidence>,
    ledger: ExportLedger,
    failures: Vec<CatalogReadFailure<'source>>,
}

pub(crate) fn read<'context, 'source>(
    forge: &'source DataForge<'source>,
    input: &'context OriginalCatalogInput<'context, 'source>,
    cancel: Option<&CancelToken>,
    dependencies: bool,
) -> Result<TagRead<'context, 'source>, CatalogError> {
    input.check_forge(forge)?;
    let mut capture = Capture {
        input,
        cancel,
        rows: TagRows::default(),
        paths: PathCapture::default(),
        relations: TagRelationReceipts::default(),
        names: BTreeMap::new(),
        ledger: ExportLedger::default(),
        failures: Vec::new(),
    };
    let mut dependency =
        dependencies.then(|| crate::export_tag_read::Capture::new(Kind::Contracts, Some(input)));
    let tree = TagTree::build_observed(
        forge,
        &|| cancel.is_some_and(CancelToken::is_cancelled),
        &mut |event| {
            capture.event(&event)?;
            if let Some(dependency) = dependency.as_mut() {
                dependency.event(&event, cancel)?;
            }
            Ok::<(), CatalogError>(())
        },
    )?
    .ok_or(CatalogError::Cancelled)?;
    let context = CapturedTags::finish(
        input,
        tree,
        capture.paths,
        capture.failures.is_empty(),
        cancel,
    )?;
    capture.rows.paths = context.path_rows(cancel)?;
    capture.rows.definitions.sort_by_key(|row| {
        (
            CatalogGuid(row.source.physical.guid).text(),
            row.source.physical.record_index,
        )
    });
    capture
        .rows
        .roots
        .sort_by_key(|row| row.source.physical.record_index);
    capture.ledger.inventory(Kind::Tags);
    let dependencies = dependency.map(|capture| {
        let (failures, ledger) = capture.finish();
        TagDependencies {
            tree: context.tree_owner(),
            failures,
            ledger,
        }
    });
    Ok(TagRead {
        dependencies,
        context,
        rows: capture.rows,
        ledger: capture.ledger,
        failures: capture.failures,
    })
}

impl<'source> Capture<'_, 'source> {
    fn event(&mut self, event: &TagBuildEvent<'_, 'source>) -> Result<(), CatalogError> {
        if self.cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        match event {
            TagBuildEvent::CanonicalRead { record, outcome } => self.canonical(record, *outcome)?,
            TagBuildEvent::MetadataFailed { record, error } => {
                self.failed(record, error, ExportSourceRole::SelectionCandidate)?;
            }
            TagBuildEvent::PathChosen {
                record,
                position,
                parent_position,
                incoming,
                depth,
                listed,
                ..
            } => {
                let name = self
                    .names
                    .remove(&record.index())
                    .ok_or(crate::export_ledger::ExportAccountingError::ConflictingTagTrace)?;
                self.paths.chosen(
                    self.input,
                    &ChosenPath {
                        record,
                        position: *position,
                        parent_position: *parent_position,
                        incoming: *incoming,
                        depth: *depth,
                        listed: *listed,
                        name,
                    },
                )?;
            }
            TagBuildEvent::RelationSkipped {
                incoming,
                target,
                reason,
            } => {
                let admission = RecordObservation::capture(self.input, incoming.record())?;
                let body =
                    self.relations
                        .take(self.input, admission.subject(), *incoming, *target)?;
                diagnostics::relation(&mut self.ledger, &admission, *reason, body)?;
            }
            TagBuildEvent::DuplicateSkipped { record } => {
                let admission = RecordObservation::capture(self.input, record)?;
                diagnostics::duplicate(self.input, &mut self.ledger, &admission)?;
            }
            TagBuildEvent::Projected { .. } => {}
            TagBuildEvent::Cancelled => return Err(CatalogError::Cancelled),
        }
        Ok(())
    }

    fn canonical(
        &mut self,
        record: &Record<'source>,
        outcome: Result<&RecordTree<'source>, &DataForgeError>,
    ) -> Result<(), CatalogError> {
        let admission = RecordObservation::capture(self.input, record)?;
        let role = if record.type_name().is_ok_and(|name| name == "TagDatabase") {
            ExportSourceRole::Dependency
        } else {
            ExportSourceRole::Selected
        };
        let tree = match outcome {
            Ok(tree) => tree,
            Err(error) => {
                self.failed(record, error, role)?;
                return Ok(());
            }
        };
        self.ledger
            .merge(crate::export_field_walk::inspect_observed(
                Some(self.input),
                tree,
                &admission,
                Some(Kind::Tags),
                role,
                self.cancel,
            )?);
        let notes = self.project_fields(record, tree)?;
        crate::source_projection_notes::record(
            Kind::Tags,
            role,
            &admission,
            tree,
            notes,
            &mut self.ledger,
        )?;
        self.ledger.observe(admission.source(
            Some(Kind::Tags),
            role,
            SourceDisposition::Projected,
        )?);
        Ok(())
    }

    fn project_fields(
        &mut self,
        record: &Record<'source>,
        tree: &RecordTree<'source>,
    ) -> Result<Vec<FieldError>, CatalogError> {
        let mut notes = Vec::new();
        let context = self.input.origins().record(tree.record_index());
        let scope = self.input.record_scope(record, tree)?;
        let original =
            crate::catalog_read::original::OriginalFlowFields::read(&scope, self.cancel)?;
        let admission = RecordObservation::capture(self.input, record)?;
        let capture = crate::template_field_capture::TemplateFieldCapture::new(
            &original, self.input, &admission,
        )?;
        let fields = CatalogFields::observed(tree.root(), "", &context, &capture)?;
        let relation_kind = if tree.type_name() == "TagDatabase" {
            TagPathEdgeKind::Root
        } else {
            TagPathEdgeKind::Child
        };
        self.relations
            .capture(relation_kind, tree, &original, capture.example_body())?;
        if tree.type_name() == "TagDatabase" {
            fields.check_observed(&["tags"], &mut notes)?;
            self.rows.roots.push(TagRoots {
                source: header::read(tree, self.input.origins())?,
                tags: fields::references(&fields, "tags", &mut notes, self.cancel)?,
            });
        } else {
            fields.check_observed(&["tagName", "legacyGUID", "children"], &mut notes)?;
            let (tag_name, name) = fields::name(&fields, &original, &mut notes)?;
            self.names.insert(record.index(), name);
            let legacy_guid = fields::legacy_guid(&fields, &mut notes)?;
            self.rows.definitions.push(TagDefinition {
                source: header::read(tree, self.input.origins())?,
                tag_name,
                legacy_guid,
                children: fields::references(&fields, "children", &mut notes, self.cancel)?,
            });
        }
        if tree.guid() == p4kx_extraction::Guid::default() {
            notes.push(
                FieldError::Malformed {
                    source_type: tree.type_name().to_owned(),
                    field: "$guid".to_owned(),
                    expected: "a non-nil Record identity",
                }
                .with_body(
                    crate::catalog_read::original::ExampleBodyReceipt::technical(
                        self.input,
                        admission.subject().clone(),
                        "$guid",
                    )?,
                ),
            );
        }
        Ok(notes)
    }

    fn failed(
        &mut self,
        record: &Record<'source>,
        error: &DataForgeError,
        role: ExportSourceRole,
    ) -> Result<(), CatalogError> {
        let admission = RecordObservation::capture(self.input, record)?;
        self.ledger.observe(
            admission
                .source(Some(Kind::Tags), role, SourceDisposition::ReadFailed)?
                .diagnosed(crate::export_source_failure::diagnosis(error)),
        );
        self.failures.push(CatalogReadFailure::of(record, error));
        Ok(())
    }
}
