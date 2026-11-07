// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod catalog_scope;
mod projection;
mod selection;

use p4kx_extraction::{CancelToken, DataForge, Record, TagTree, TreeBuilder};

use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDiagnosis, SourceDisposition, SourceObservation};
use crate::{CatalogError, CatalogFailure, Kind};

use super::Piece;
use selection::Action;

pub(super) use catalog_scope::needed as catalog_needs_index;
pub(crate) use projection::title_key;

/// A mission dependency's actual read and typed projection.
pub(super) struct ReadPiece<'f> {
    pub(super) piece: Option<Piece<'f>>,
    pub(super) ledger: ExportLedger,
}

fn observe(
    ledger: &mut ExportLedger,
    action: Action,
    record: &Record<'_>,
    disposition: SourceDisposition,
    diagnosis: &SourceDiagnosis,
) {
    for &kind in action.kinds() {
        let role = if kind == Kind::Contracts {
            ExportSourceRole::Dependency
        } else {
            ExportSourceRole::DerivedInput
        };
        ledger.observe(
            SourceObservation::new(kind, record, role, disposition).diagnosed(diagnosis.clone()),
        );
    }
}

fn failed<'f>(
    mut ledger: ExportLedger,
    action: Action,
    record: &Record<'f>,
    disposition: SourceDisposition,
    error: &(dyn std::error::Error + 'static),
) -> ReadPiece<'f> {
    observe(
        &mut ledger,
        action,
        record,
        disposition,
        &crate::export_source_failure::diagnosis(error),
    );
    ReadPiece {
        piece: Some(Piece::Failed(CatalogFailure::of(record, error))),
        ledger,
    }
}

fn candidate<'f>(record: &Record<'f>, error: &p4kx_extraction::DataForgeError) -> ReadPiece<'f> {
    let mut ledger = ExportLedger::default();
    ledger.observe(
        SourceObservation::new(
            Kind::Contracts,
            record,
            ExportSourceRole::SelectionCandidate,
            SourceDisposition::ReadFailed,
        )
        .diagnosed(crate::export_source_failure::diagnosis(error)),
    );
    ReadPiece {
        piece: Some(Piece::Failed(CatalogFailure::of(record, error))),
        ledger,
    }
}

pub(super) fn read<'f>(
    trees: &mut TreeBuilder<'f>,
    record: &Record<'f>,
    tags: &TagTree,
    cancel: Option<&CancelToken>,
) -> Result<Option<ReadPiece<'f>>, CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    let action = match selection::choose(record) {
        Ok(Some(action)) => action,
        Ok(None) => return Ok(None),
        Err(error) => return Ok(Some(candidate(record, &error))),
    };
    let tree = match trees.build(record) {
        Ok(tree) => tree,
        Err(error) => {
            return Ok(Some(failed(
                ExportLedger::default(),
                action,
                record,
                SourceDisposition::ReadFailed,
                &error,
            )));
        }
    };
    let mut ledger = crate::export_field_walk::inspect(
        &tree,
        record,
        Some(Kind::Contracts),
        ExportSourceRole::Dependency,
        cancel,
    )?;
    let piece = match projection::project(action, &tree, tags) {
        Ok(piece) => piece,
        Err(error) => {
            return Ok(Some(failed(
                ledger,
                action,
                record,
                SourceDisposition::ProjectionFailed,
                &error,
            )));
        }
    };
    observe(
        &mut ledger,
        action,
        record,
        SourceDisposition::Projected,
        &SourceDiagnosis::default(),
    );
    Ok(Some(ReadPiece { piece, ledger }))
}

pub(super) fn read_tags(
    forge: &DataForge<'_>,
    cancel: Option<&CancelToken>,
) -> Result<(TagTree, Vec<CatalogFailure>, ExportLedger), CatalogError> {
    let tags = crate::export_tag_read::read(forge, Kind::Contracts, cancel)?;
    Ok((tags.tree, tags.failures, tags.ledger))
}

#[cfg(test)]
mod tests;
