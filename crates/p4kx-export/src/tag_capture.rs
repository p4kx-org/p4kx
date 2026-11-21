// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::{
    CancelToken, DataForge, DataForgeError, Guid, Record, RecordTree, TAG_DATABASE_TYPE,
    TagBuildEvent, TagPathIncoming, TagRelationIssue, TagTree,
};

use super::contract::{TagDefinition, TagRoots, TagRows};
use super::declaration::{self, Declaration};
use super::diagnostics;
use super::path_capture::{ChosenPath, NameEvidence, PathCapture};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::TagRelationReceipts;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::export_tag_read::Capture as DependencyCapture;
use crate::health_diagnostic_observation::ExampleObservation;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, CatalogReadFailure, Kind};

#[path = "captured_tags.rs"]
mod captured;

pub use captured::OriginalTagPath;
pub(crate) use captured::{
    CapturedTagTree, CapturedTags, ContextualTagPath, TagDatum, TagNameSelector, TagPathLookup,
    TagPathMeaning,
};

pub(crate) struct TagRead<'context, 'source> {
    pub(crate) rows: TagRows<'source>,
    pub(crate) context: CapturedTags<'context, 'source>,
    pub(crate) dependencies: Option<TagDependencies<'source>>,
    pub(crate) paint_dependencies: Option<TagDependencies<'source>>,
    pub(crate) ledger: ExportLedger,
    pub(crate) failures: Vec<CatalogReadFailure<'source>>,
}

/// The original dependency observations from the same Tag build.
pub(crate) struct TagDependencies<'source> {
    pub(crate) tree: CapturedTagTree,
    pub(crate) failures: Vec<CatalogReadFailure<'source>>,
    pub(crate) ledger: ExportLedger,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct TagConsumers {
    pub(crate) contracts: bool,
    pub(crate) paints: bool,
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
    consumers: TagConsumers,
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
    let mut dependencies = [
        consumers
            .contracts
            .then(|| DependencyCapture::new(Kind::Contracts, Some(input))),
        consumers
            .paints
            .then(|| DependencyCapture::new(Kind::Paints, Some(input))),
    ];
    let tree = TagTree::build_observed(
        forge,
        &|| cancel.is_some_and(CancelToken::is_cancelled),
        &mut |event| capture.event(&event, &mut dependencies),
    )?
    .ok_or(CatalogError::Cancelled)?;
    drop(capture.relations);
    drop(capture.names);
    let context = CapturedTags::finish(
        input,
        tree,
        capture.paths,
        capture.failures.is_empty(),
        cancel,
    )?;
    finish_rows(&mut capture.rows, &context, cancel)?;
    capture.ledger.inventory(Kind::Tags);
    let [dependencies, paint_dependencies] = dependencies.map(|dependency| {
        dependency.map(|capture| {
            let (failures, ledger) = capture.finish();
            TagDependencies {
                tree: context.tree_owner(),
                failures,
                ledger,
            }
        })
    });
    Ok(TagRead {
        dependencies,
        paint_dependencies,
        context,
        rows: capture.rows,
        ledger: capture.ledger,
        failures: capture.failures,
    })
}

fn finish_rows(
    rows: &mut TagRows<'_>,
    context: &CapturedTags<'_, '_>,
    cancel: Option<&CancelToken>,
) -> Result<(), CatalogError> {
    rows.paths = context.path_rows(cancel)?;
    rows.definitions.sort_by_key(TagDefinition::order_key);
    rows.roots.sort_by_key(TagRoots::order_key);
    Ok(())
}

impl<'source> Capture<'_, 'source> {
    fn event(
        &mut self,
        event: &TagBuildEvent<'_, 'source>,
        dependencies: &mut [Option<DependencyCapture<'_, 'source>>],
    ) -> Result<(), CatalogError> {
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
                return self.skipped_relation(*incoming, *target, *reason, dependencies);
            }
            TagBuildEvent::DuplicateSkipped { record } => {
                let admission = RecordObservation::capture(self.input, record)?;
                diagnostics::duplicate(self.input, &mut self.ledger, &admission)?;
            }
            TagBuildEvent::Projected { .. } => {}
            TagBuildEvent::Cancelled => return Err(CatalogError::Cancelled),
        }
        for dependency in dependencies.iter_mut().flatten() {
            dependency.event(event)?;
        }
        Ok(())
    }

    fn skipped_relation(
        &mut self,
        incoming: TagPathIncoming<'_, 'source>,
        target: Option<Guid>,
        reason: TagRelationIssue,
        dependencies: &mut [Option<DependencyCapture<'_, 'source>>],
    ) -> Result<(), CatalogError> {
        let admission = RecordObservation::capture(self.input, incoming.record())?;
        let body = self
            .relations
            .take(self.input, admission.subject(), incoming, target)?;
        let (field, source_type, target_guid) = body.description()?;
        let example = ExampleObservation::captured(
            admission.subject().clone(),
            field.as_deref(),
            source_type.as_deref(),
            target_guid,
            body,
        )?;
        for dependency in dependencies.iter_mut().flatten() {
            dependency.relation(reason, example.clone());
        }
        diagnostics::relation(&mut self.ledger, &admission, reason, example);
        Ok(())
    }

    fn canonical(
        &mut self,
        record: &Record<'source>,
        outcome: Result<&RecordTree<'source>, &DataForgeError>,
    ) -> Result<(), CatalogError> {
        let admission = RecordObservation::capture(self.input, record)?;
        let role = if record
            .type_name()
            .is_ok_and(|name| name == TAG_DATABASE_TYPE)
        {
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
        let projected = declaration::read_declaration(
            tree,
            self.input,
            &admission,
            Some(&mut self.relations),
            self.cancel,
        )?;
        if let Some(name) = projected.path_name {
            self.names.insert(record.index(), name);
        }
        match projected.row {
            Declaration::Definition(row) => self.rows.definitions.push(row),
            Declaration::Roots(row) => self.rows.roots.push(row),
        }
        crate::source_projection_notes::record(
            Kind::Tags,
            role,
            &admission,
            tree,
            projected.notes,
            &mut self.ledger,
        )?;
        self.ledger.observe(admission.source(
            Some(Kind::Tags),
            role,
            SourceDisposition::Projected,
        )?);
        Ok(())
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

#[cfg(test)]
#[path = "tag_capture_tests.rs"]
mod tests;
