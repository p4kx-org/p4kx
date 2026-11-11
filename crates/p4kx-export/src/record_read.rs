// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter, Record, RecordTree, TreeBuilder};
use rayon::prelude::*;

use crate::build::original_input::OriginalCatalogInput;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition, SourceObservation};
use crate::guid_text::{CatalogGuid, sort_by_guid};
use crate::record_observation::RecordObservation;
use crate::selection::{Kind, is_kind_leftover};
use crate::{CatalogError, CatalogReadFailure};

/// The entities of one kind of a DataForge and what was left out.
pub(crate) struct KindRead<'f, T> {
    pub(crate) items: Vec<T>,
    pub(crate) skipped: usize,
    pub(crate) failures: Vec<CatalogReadFailure<'f>>,
    pub(crate) ledger: ExportLedger,
}

/// A Record that cannot be read: the error of the DataForge or of an identifier of a description.
pub(crate) type ReadError = Box<dyn std::error::Error + Send + Sync>;

enum Piece<'f, T> {
    Item(Box<T>),
    Skipped,
    Failed(CatalogReadFailure<'f>),
}

struct Observed<'f, T> {
    piece: Piece<'f, T>,
    source: SourceObservation,
    fields: ExportLedger,
}

/// Reads the Records of `kind` that `filter` matches with `build`, ordered by the GUID `guid`
/// returns; a development leftover is left out and counted, a Record that cannot be read is
/// reported as a failure.
///
/// # Errors
///
/// Returns [`CatalogError::Cancelled`] once `cancel` is cancelled, or a source-evidence error
/// when the Forge or canonical read does not belong to the admitted input.
pub(crate) fn read_kind<'f, T: Send>(
    forge: &'f DataForge<'f>,
    input: &OriginalCatalogInput<'_, '_>,
    kind: Kind,
    (filter, cancel): (Option<&Filter>, Option<&CancelToken>),
    build: impl Fn(&Record<'f>, &RecordTree<'f>) -> Result<T, ReadError> + Sync,
    guid: impl Fn(&T) -> CatalogGuid,
) -> Result<KindRead<'f, T>, CatalogError> {
    input.check_forge(forge)?;
    let rule = kind.records();
    let records: Vec<Record<'f>> = forge
        .records()
        .filter(|record| {
            record
                .record_path()
                .is_ok_and(|path| rule.matches_path(path) && filter.is_none_or(|f| f.matches(path)))
        })
        .collect();
    let pieces: Vec<Observed<'f, T>> = records
        .par_iter()
        .map_init(TreeBuilder::new, |trees, record| {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            piece(input, kind, trees, record, &build, cancel)
        })
        .filter_map(Result::transpose)
        .collect::<Result<_, _>>()?;
    let mut read = KindRead {
        items: Vec::new(),
        skipped: 0,
        failures: Vec::new(),
        ledger: ExportLedger::default(),
    };
    read.ledger.inventory(kind);
    for observed in pieces {
        read.ledger.observe(observed.source);
        read.ledger.merge(observed.fields);
        match observed.piece {
            Piece::Item(item) => read.items.push(*item),
            Piece::Skipped => read.skipped = read.skipped.saturating_add(1),
            Piece::Failed(failure) => read.failures.push(failure),
        }
    }
    sort_by_guid(&mut read.items, guid);
    Ok(read)
}

fn piece<'f, T>(
    input: &OriginalCatalogInput<'_, '_>,
    kind: Kind,
    trees: &mut TreeBuilder<'f>,
    record: &Record<'f>,
    build: &impl Fn(&Record<'f>, &RecordTree<'f>) -> Result<T, ReadError>,
    cancel: Option<&CancelToken>,
) -> Result<Option<Observed<'f, T>>, CatalogError> {
    let rule = kind.records();
    let selection: Result<Option<bool>, p4kx_extraction::DataForgeError> = (|| {
        let path = record.record_path()?;
        if record.type_name()? != rule.type_name {
            return Ok(None);
        }
        Ok(Some(is_kind_leftover(kind, path, record.name()?)))
    })();
    let picked = match selection {
        Ok(None) => return Ok(None),
        Ok(Some(false)) => return project(input, kind, trees, record, build, cancel).map(Some),
        Ok(Some(true)) => {
            let admission = RecordObservation::capture(input, record)?;
            Observed {
                piece: Piece::Skipped,
                source: admission.source(
                    Some(kind),
                    ExportSourceRole::Selected,
                    SourceDisposition::SkippedBeforeRead,
                )?,
                fields: ExportLedger::default(),
            }
        }
        Err(error) => {
            let admission = RecordObservation::capture(input, record)?;
            failed(
                kind,
                &admission,
                &error,
                ExportSourceRole::SelectionCandidate,
                SourceDisposition::ReadFailed,
            )?
        }
    };
    Ok(Some(picked))
}

fn project<'f, T>(
    input: &OriginalCatalogInput<'_, '_>,
    kind: Kind,
    trees: &mut TreeBuilder<'f>,
    record: &Record<'f>,
    build: &impl Fn(&Record<'f>, &RecordTree<'f>) -> Result<T, ReadError>,
    cancel: Option<&CancelToken>,
) -> Result<Observed<'f, T>, CatalogError> {
    let admission = RecordObservation::capture(input, record)?;
    let tree = match trees.build(record) {
        Ok(tree) => tree,
        Err(error) => {
            return failed(
                kind,
                &admission,
                &error,
                ExportSourceRole::Selected,
                SourceDisposition::ReadFailed,
            );
        }
    };
    let fields = crate::export_field_walk::inspect_observed(
        Some(input),
        &tree,
        &admission,
        Some(kind),
        ExportSourceRole::Selected,
        cancel,
    )?;
    let mut observed = match build(record, &tree) {
        Ok(item) => Observed {
            piece: Piece::Item(Box::new(item)),
            source: admission.source(
                Some(kind),
                ExportSourceRole::Selected,
                SourceDisposition::Projected,
            )?,
            fields: ExportLedger::default(),
        },
        Err(error) => match error.downcast::<CatalogError>() {
            Ok(error) => return Err(*error),
            Err(error) => failed(
                kind,
                &admission,
                error.as_ref(),
                ExportSourceRole::Selected,
                SourceDisposition::ProjectionFailed,
            )?,
        },
    };
    observed.fields = fields;
    Ok(observed)
}

fn failed<'f, T>(
    kind: Kind,
    admission: &RecordObservation<'_, 'f>,
    error: &(dyn std::error::Error + 'static),
    role: ExportSourceRole,
    disposition: SourceDisposition,
) -> Result<Observed<'f, T>, CatalogError> {
    Ok(Observed {
        piece: Piece::Failed(CatalogReadFailure::of(admission.record(), error)),
        source: admission
            .source(Some(kind), role, disposition)?
            .diagnosed(crate::export_source_failure::diagnosis(error)),
        fields: ExportLedger::default(),
    })
}
#[cfg(test)]
#[path = "record_read_original_tests.rs"]
mod original_tests;

#[cfg(test)]
#[path = "record_read_callback_error_tests.rs"]
mod callback_error_tests;

#[cfg(test)]
#[path = "record_read_path_tests.rs"]
mod path_boundary_tests;
