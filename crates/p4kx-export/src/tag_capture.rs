// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::{
    CancelToken, DataForge, DataForgeError, Record, RecordTree, SourceValueState, TagBuildEvent,
    TagTree,
};

use super::contract::{TagDefinition, TagRoots, TagRows};
use super::path_capture::{ChosenPath, PathCapture};
use super::{diagnostics, fields, header};
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::CatalogFields;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition, SourceObservation};
use crate::guid_text::CatalogGuid;
use crate::source_fields::FieldError;
use crate::{CatalogError, CatalogFailure, Kind};

pub(crate) struct TagRead<'source> {
    pub(crate) rows: TagRows<'source>,
    pub(crate) ledger: ExportLedger,
    pub(crate) failures: Vec<CatalogFailure>,
}

struct Capture<'context, 'source> {
    origins: &'context CatalogOrigins,
    cancel: Option<&'context CancelToken>,
    rows: TagRows<'source>,
    paths: PathCapture,
    states: BTreeMap<u32, SourceValueState>,
    ledger: ExportLedger,
    failures: Vec<CatalogFailure>,
}

pub(crate) fn read<'source>(
    forge: &'source DataForge<'source>,
    origins: &CatalogOrigins,
    cancel: Option<&CancelToken>,
) -> Result<TagRead<'source>, CatalogError> {
    let mut capture = Capture {
        origins,
        cancel,
        rows: TagRows::default(),
        paths: PathCapture::default(),
        states: BTreeMap::new(),
        ledger: ExportLedger::default(),
        failures: Vec::new(),
    };
    let tree = TagTree::build_observed(
        forge,
        &|| cancel.is_some_and(CancelToken::is_cancelled),
        &mut |event| capture.event(&event),
    )?
    .ok_or(CatalogError::Cancelled)?;
    drop(tree);
    capture.rows.paths = capture.paths.rows;
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
    Ok(TagRead {
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
                self.failed(record, error, ExportSourceRole::SelectionCandidate);
            }
            TagBuildEvent::PathChosen {
                record,
                position,
                parent_position,
                incoming,
                path,
                depth,
                listed,
            } => {
                let name_state = self
                    .states
                    .get(&record.index())
                    .copied()
                    .ok_or(crate::export_ledger::ExportAccountingError::ConflictingTagTrace)?;
                self.paths.chosen(
                    self.origins,
                    &ChosenPath {
                        record_index: record.index(),
                        guid: record.guid(),
                        position: *position,
                        parent_position: *parent_position,
                        incoming: incoming.map(|edge| {
                            (edge.record().index(), edge.kind().field(), edge.ordinal())
                        }),
                        path,
                        depth: *depth,
                        listed: *listed,
                        name_state,
                    },
                )?;
            }
            TagBuildEvent::ProjectionFailed { record, reason } => {
                diagnostics::cut(&mut self.ledger, record, *reason);
            }
            TagBuildEvent::RelationSkipped {
                record,
                field,
                target,
                reason,
            } => diagnostics::relation(&mut self.ledger, record, field, *target, *reason),
            TagBuildEvent::DuplicateSkipped { record } => {
                diagnostics::duplicate(&mut self.ledger, record);
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
        let role = if record.type_name().is_ok_and(|name| name == "TagDatabase") {
            ExportSourceRole::Dependency
        } else {
            ExportSourceRole::Selected
        };
        let tree = match outcome {
            Ok(tree) => tree,
            Err(error) => {
                self.failed(record, error, role);
                return Ok(());
            }
        };
        self.ledger.merge(crate::export_field_walk::inspect(
            tree,
            record,
            Some(Kind::Tags),
            role,
            self.cancel,
        )?);
        let mut notes = Vec::new();
        let context = self.origins.record(tree.record_index());
        let fields = CatalogFields::new(tree.root(), "", &context);
        if tree.type_name() == "TagDatabase" {
            fields.check(&["tags"], &mut notes);
            self.rows.roots.push(TagRoots {
                source: header::read(tree, self.origins)?,
                tags: fields::references(&fields, "tags", &mut notes, self.cancel)?,
            });
        } else {
            fields.check(&["tagName", "legacyGUID", "children"], &mut notes);
            let tag_name = fields.text("tagName", &mut notes)?;
            self.states.insert(record.index(), tag_name.state());
            let legacy_guid = fields::legacy_guid(&fields, &mut notes)?;
            self.rows.definitions.push(TagDefinition {
                source: header::read(tree, self.origins)?,
                tag_name,
                legacy_guid,
                children: fields::references(&fields, "children", &mut notes, self.cancel)?,
            });
        }
        if tree.guid() == p4kx_extraction::Guid::default() {
            notes.push(FieldError::Malformed {
                source_type: tree.type_name().to_owned(),
                field: "$guid".to_owned(),
                expected: "a non-nil Record identity",
            });
        }
        crate::source_projection_notes::record(Kind::Tags, role, tree, notes, &mut self.ledger);
        self.ledger.observe(SourceObservation::new(
            Kind::Tags,
            record,
            role,
            SourceDisposition::Projected,
        ));
        Ok(())
    }

    fn failed(&mut self, record: &Record<'_>, error: &DataForgeError, role: ExportSourceRole) {
        self.ledger.observe(
            SourceObservation::new(Kind::Tags, record, role, SourceDisposition::ReadFailed)
                .diagnosed(crate::export_source_failure::diagnosis(error)),
        );
        self.failures.push(CatalogFailure::of(record, error));
    }
}
