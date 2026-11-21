// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, DataForge, Record, RecordTree, TagBuildEvent, TagPathEdgeKind, TagTree,
};

use super::Capture;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{OriginalFlowFields, TagRelationReceipts};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::ExportLedger;
use crate::health_diagnostic_observation::ExampleObservation;
use crate::{CatalogError, CatalogReadFailure, Kind};

pub(crate) struct TagRead<'source> {
    pub(crate) tree: TagTree,
    pub(crate) ledger: ExportLedger,
    pub(crate) failures: Vec<CatalogReadFailure<'source>>,
}

pub(crate) fn read<'source>(
    forge: &'source DataForge<'source>,
    kind: Kind,
    input: Option<&OriginalCatalogInput<'_, 'source>>,
    cancel: Option<&CancelToken>,
) -> Result<TagRead<'source>, CatalogError> {
    let mut capture = Capture::new(kind, input);
    let mut relations = TagRelationReceipts::default();
    let tree = TagTree::build_observed(
        forge,
        &|| cancel.is_some_and(CancelToken::is_cancelled),
        &mut |event| {
            capture.event(&event)?;
            match event {
                TagBuildEvent::CanonicalRead {
                    record,
                    outcome: Ok(tree),
                } => {
                    inspect(&mut capture, &mut relations, record, tree, cancel)?;
                }
                TagBuildEvent::RelationSkipped {
                    incoming,
                    target,
                    reason,
                } => {
                    let admission = capture.admission(incoming.record())?;
                    let example = if let Some(input) = input {
                        let body = relations.take(input, admission.subject(), incoming, target)?;
                        let (field, source_type, target_guid) = body.description()?;
                        ExampleObservation::captured(
                            admission.subject().clone(),
                            field.as_deref(),
                            source_type.as_deref(),
                            target_guid,
                            body,
                        )?
                    } else {
                        ExampleObservation::new(
                            admission.subject().clone(),
                            Some(incoming.kind().field()),
                            admission.record().type_name().ok(),
                            target,
                        )
                    };
                    capture.relation(reason, example);
                }
                TagBuildEvent::CanonicalRead {
                    outcome: Err(_), ..
                }
                | TagBuildEvent::MetadataFailed { .. }
                | TagBuildEvent::Projected { .. }
                | TagBuildEvent::DuplicateSkipped { .. }
                | TagBuildEvent::PathChosen { .. }
                | TagBuildEvent::Cancelled => {}
            }
            Ok::<(), CatalogError>(())
        },
    )?
    .ok_or(CatalogError::Cancelled)?;
    let (failures, ledger) = capture.finish();
    Ok(TagRead {
        tree,
        ledger,
        failures,
    })
}

fn inspect(
    capture: &mut Capture<'_, '_>,
    relations: &mut TagRelationReceipts,
    record: &Record<'_>,
    tree: &RecordTree<'_>,
    cancel: Option<&CancelToken>,
) -> Result<(), CatalogError> {
    let admission = capture.admission(record)?;
    if let Some(input) = capture.input {
        let scope = input.record_scope(record, tree)?;
        let original = OriginalFlowFields::read(&scope, cancel)?;
        let body = original.body_capture(input, &admission)?;
        let edge = if tree.type_name() == "TagDatabase" {
            TagPathEdgeKind::Root
        } else {
            TagPathEdgeKind::Child
        };
        relations.capture(edge, tree, &original, &body)?;
    }
    capture
        .ledger
        .merge(crate::export_field_walk::inspect_observed(
            capture.input,
            tree,
            &admission,
            Some(capture.kind),
            ExportSourceRole::Dependency,
            cancel,
        )?);
    Ok(())
}
