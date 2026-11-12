// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod catalog_scope;
mod projection;
mod selection;

#[cfg(test)]
use p4kx_extraction::DataForge;
use p4kx_extraction::{CancelToken, Record, TagTree, TreeBuilder};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{OriginalFlowFields, RewardRead};
use crate::catalog_v3::tag_catalog::CapturedTags;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDiagnosis, SourceDisposition};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, CatalogReadFailure, Kind};

use super::template_read::FailedTemplate;
use super::{Piece, RewardDependency};
use selection::Action;

pub(super) use catalog_scope::needed as catalog_needs_index;

pub(super) fn reward_candidate(
    record: &Record<'_>,
) -> Result<bool, p4kx_extraction::DataForgeError> {
    Ok(selection::choose(record)? == Some(Action::Reward))
}

/// A mission dependency's actual read and typed projection.
pub(super) struct ReadPiece<'f> {
    pub(super) piece: Option<Piece<'f>>,
    pub(super) ledger: ExportLedger,
}

fn observe(
    ledger: &mut ExportLedger,
    action: Action,
    admission: &RecordObservation<'_, '_>,
    disposition: SourceDisposition,
    diagnosis: &SourceDiagnosis,
) -> Result<(), CatalogError> {
    for &kind in action.kinds() {
        let role = if kind == Kind::Contracts {
            ExportSourceRole::Dependency
        } else {
            ExportSourceRole::DerivedInput
        };
        ledger.observe(
            admission
                .source(Some(kind), role, disposition)?
                .diagnosed(diagnosis.clone()),
        );
    }
    Ok(())
}

fn failed<'f>(
    mut ledger: ExportLedger,
    action: Action,
    admission: &RecordObservation<'_, 'f>,
    disposition: SourceDisposition,
    error: &(dyn std::error::Error + 'static),
    input: Option<&OriginalCatalogInput<'_, '_>>,
) -> Result<ReadPiece<'f>, CatalogError> {
    let record = admission.record();
    observe(
        &mut ledger,
        action,
        admission,
        disposition,
        &crate::export_source_failure::diagnosis(error),
    )?;
    let failure = CatalogReadFailure::of(record, error);
    let piece = match (action, disposition, input) {
        (Action::Reward, SourceDisposition::ReadFailed, Some(input)) => {
            let reward = RewardRead::failed(input, admission, &failure)?;
            Piece::UnusableRewardCandidate(failure, RewardDependency::Original(reward))
        }
        (Action::Template, SourceDisposition::ReadFailed, Some(input)) => {
            let attempt = FailedTemplate::read(input, admission, &failure)?;
            Piece::FailedTemplate(failure, attempt)
        }
        _ => Piece::Failed(failure),
    };
    Ok(ReadPiece {
        piece: Some(piece),
        ledger,
    })
}

fn candidate<'f>(
    admission: &RecordObservation<'_, 'f>,
    error: &p4kx_extraction::DataForgeError,
    unreadable_path: Option<RewardRead>,
) -> Result<ReadPiece<'f>, CatalogError> {
    let mut ledger = ExportLedger::default();
    ledger.observe(
        admission
            .source(
                Some(Kind::Contracts),
                ExportSourceRole::SelectionCandidate,
                SourceDisposition::ReadFailed,
            )?
            .diagnosed(crate::export_source_failure::diagnosis(error)),
    );
    let failure = CatalogReadFailure::of(admission.record(), error);
    let piece = match unreadable_path {
        Some(path) => {
            Piece::UnusableRewardCandidate(failure, RewardDependency::UnreadablePath(path))
        }
        None => Piece::Failed(failure),
    };
    Ok(ReadPiece {
        piece: Some(piece),
        ledger,
    })
}
fn admit<'read, 'source>(
    record: &'read Record<'source>,
    input: Option<&OriginalCatalogInput<'_, '_>>,
) -> Result<RecordObservation<'read, 'source>, CatalogError> {
    input.map_or_else(
        || Ok(RecordObservation::accounting(record)),
        |input| RecordObservation::capture(input, record),
    )
}

pub(super) fn read<'f>(
    trees: &mut TreeBuilder<'f>,
    record: &Record<'f>,
    tags: &TagTree,
    input: Option<&OriginalCatalogInput<'_, '_>>,
    captured_tags: Option<&CapturedTags<'_, 'f>>,
    cancel: Option<&CancelToken>,
) -> Result<Option<ReadPiece<'f>>, CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    let admission = admit(record, input)?;
    let path = match record.record_path() {
        Ok(path) => path,
        Err(error) => {
            let unreadable = input
                .map(|input| RewardRead::unreadable_path(input, &admission))
                .transpose()?;
            return candidate(&admission, &error, unreadable).map(Some);
        }
    };
    let action = match selection::choose_at_path(record, path) {
        Ok(Some(action)) => action,
        Ok(None) => return Ok(None),
        Err(error) => return candidate(&admission, &error, None).map(Some),
    };
    let tree = match trees.build(record) {
        Ok(tree) => tree,
        Err(error) => {
            return failed(
                ExportLedger::default(),
                action,
                &admission,
                SourceDisposition::ReadFailed,
                &error,
                input,
            )
            .map(Some);
        }
    };
    let ledger = inspect_dependency(action, input, &tree, &admission, cancel)?;
    let projected = project(
        action,
        &admission,
        &tree,
        tags,
        input,
        captured_tags,
        cancel,
    );
    finish_projection(projected, ledger, action, &admission, input).map(Some)
}

fn inspect_dependency<'source>(
    action: Action,
    input: Option<&OriginalCatalogInput<'_, '_>>,
    tree: &p4kx_extraction::RecordTree<'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ExportLedger, CatalogError> {
    if let (Action::Faction, Some(input)) = (action, input) {
        crate::export_field_walk::inspect_record_field(
            input,
            tree,
            admission,
            Kind::Contracts,
            projection::FACTION_DISPLAY_NAME,
            cancel,
        )
    } else {
        crate::export_field_walk::inspect_observed(
            input,
            tree,
            admission,
            Some(Kind::Contracts),
            ExportSourceRole::Dependency,
            cancel,
        )
    }
}

fn finish_projection<'source>(
    projected: Result<Option<Piece<'source>>, crate::record_read::ReadError>,
    mut ledger: ExportLedger,
    action: Action,
    admission: &RecordObservation<'_, 'source>,
    input: Option<&OriginalCatalogInput<'_, '_>>,
) -> Result<ReadPiece<'source>, CatalogError> {
    let piece = match projected {
        Ok(piece) => piece,
        Err(error) => {
            let error = match error.downcast::<CatalogError>() {
                Ok(error) => return Err(*error),
                Err(error) => error,
            };
            return failed(
                ledger,
                action,
                admission,
                SourceDisposition::ProjectionFailed,
                error.as_ref(),
                input,
            );
        }
    };
    observe(
        &mut ledger,
        action,
        admission,
        SourceDisposition::Projected,
        &SourceDiagnosis::default(),
    )?;
    Ok(ReadPiece { piece, ledger })
}

fn project<'source>(
    action: Action,
    admission: &RecordObservation<'_, 'source>,
    tree: &p4kx_extraction::RecordTree<'source>,
    tags: &TagTree,
    input: Option<&OriginalCatalogInput<'_, '_>>,
    captured_tags: Option<&CapturedTags<'_, 'source>>,
    cancel: Option<&CancelToken>,
) -> Result<Option<Piece<'source>>, crate::record_read::ReadError> {
    let scope = if matches!(
        action,
        Action::Template | Action::Broker | Action::Organization
    ) {
        input
            .map(|input| input.record_scope(admission.record(), tree))
            .transpose()?
    } else {
        None
    };
    let fields = scope
        .as_ref()
        .map(|scope| OriginalFlowFields::read(scope, cancel))
        .transpose()?;
    let texts = input
        .zip(fields.as_ref())
        .map(|(input, fields)| projection::TextInputs {
            fields,
            origins: input.origins(),
            tags: captured_tags,
        });
    projection::project(
        action,
        tree,
        texts,
        |pools| match (input, captured_tags, fields.as_ref()) {
            (Some(input), Some(captured), Some(fields)) => crate::template_dependency_spawns::read(
                input, captured, admission, tree, fields, pools,
            ),
            (None, None, None) => crate::spawn_layers::Level::read(
                crate::spawn_model::SpawnLevel::Template,
                &crate::spawn_read::Reader::for_record(tags, tree),
                (Some(tree.root()), "contractProperties"),
                (None, ""),
                pools,
            ),
            _ => Err(Box::new(CatalogError::from(
                p4kx_extraction::SourceEvidenceError::ForeignEvidence,
            )) as crate::record_read::ReadError),
        },
        || match input {
            Some(input) => Ok(RewardDependency::Original(RewardRead::from_tree(
                input, admission, tree, cancel,
            )?)),
            #[cfg(test)]
            None => Ok(RewardDependency::Accounting(crate::record_fields::integer(
                Some(tree.root()),
                "reputationAmount",
            ))),
            #[cfg(not(test))]
            None => Err(Box::new(CatalogError::from(
                crate::ExportAccountingError::UnboundHealthSubject,
            ))),
        },
    )
}

#[cfg(test)]
pub(super) fn read_tags<'f>(
    forge: &'f DataForge<'f>,
    input: Option<&OriginalCatalogInput<'_, 'f>>,
    cancel: Option<&CancelToken>,
) -> Result<(TagTree, Vec<CatalogReadFailure<'f>>, ExportLedger), CatalogError> {
    let tags = crate::export_tag_read::read(forge, Kind::Contracts, input, cancel)?;
    Ok((tags.tree, tags.failures, tags.ledger))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "template_number_dependency_tests.rs"]
mod number_dependency_tests;
