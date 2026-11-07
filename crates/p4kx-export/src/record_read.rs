// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter, Record, RecordTree, TreeBuilder};
use rayon::prelude::*;

use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition, SourceObservation};
use crate::guid_text::{CatalogGuid, sort_by_guid};
use crate::selection::{Kind, is_kind_leftover};
use crate::{CatalogError, CatalogFailure};

/// The entities of one kind of a DataForge and what was left out.
pub(crate) struct KindRead<T> {
    pub(crate) items: Vec<T>,
    pub(crate) skipped: usize,
    pub(crate) failures: Vec<CatalogFailure>,
    pub(crate) ledger: ExportLedger,
}

/// A Record that cannot be read: the error of the DataForge or of an identifier of a description.
pub(crate) type ReadError = Box<dyn std::error::Error + Send + Sync>;

enum Piece<T> {
    Item(Box<T>),
    Skipped,
    Failed(CatalogFailure),
}

struct Observed<T> {
    piece: Piece<T>,
    source: SourceObservation,
    fields: ExportLedger,
}

/// Reads the Records of `kind` that `filter` matches with `build`, ordered by the GUID `guid`
/// returns; a development leftover is left out and counted, a Record that cannot be read is
/// reported as a failure.
///
/// # Errors
///
/// Returns [`CatalogError::Cancelled`] once `cancel` is cancelled.
pub(crate) fn read_kind<'f, T: Send>(
    forge: &'f DataForge<'f>,
    kind: Kind,
    (filter, cancel): (Option<&Filter>, Option<&CancelToken>),
    build: impl Fn(&RecordTree<'f>) -> Result<T, ReadError> + Sync,
    guid: impl Fn(&T) -> CatalogGuid,
) -> Result<KindRead<T>, CatalogError> {
    let rule = kind.records();
    let records: Vec<Record<'f>> = forge
        .records()
        .filter(|record| {
            record
                .record_path()
                .is_ok_and(|path| rule.matches_path(path) && filter.is_none_or(|f| f.matches(path)))
        })
        .collect();
    let pieces: Vec<Observed<T>> = records
        .par_iter()
        .map_init(TreeBuilder::new, |trees, record| {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            piece(kind, trees, record, &build, cancel)
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
    kind: Kind,
    trees: &mut TreeBuilder<'f>,
    record: &Record<'f>,
    build: &impl Fn(&RecordTree<'f>) -> Result<T, ReadError>,
    cancel: Option<&CancelToken>,
) -> Result<Option<Observed<T>>, CatalogError> {
    let rule = kind.records();
    let selection: Result<Option<bool>, p4kx_extraction::DataForgeError> = (|| {
        let path = record.record_path()?;
        if record.type_name()? != rule.type_name {
            return Ok(None);
        }
        Ok(Some(is_kind_leftover(kind, path, record.name()?)))
    })();
    Ok(match selection {
        Ok(None) => None,
        Ok(Some(true)) => Some(Observed {
            piece: Piece::Skipped,
            source: SourceObservation::new(
                kind,
                record,
                ExportSourceRole::Selected,
                SourceDisposition::SkippedBeforeRead,
            ),
            fields: ExportLedger::default(),
        }),
        Ok(Some(false)) => Some(project(kind, trees, record, build, cancel)?),
        Err(error) => Some(failed(
            kind,
            record,
            &error,
            ExportSourceRole::SelectionCandidate,
            SourceDisposition::ReadFailed,
        )),
    })
}

fn project<'f, T>(
    kind: Kind,
    trees: &mut TreeBuilder<'f>,
    record: &Record<'f>,
    build: &impl Fn(&RecordTree<'f>) -> Result<T, ReadError>,
    cancel: Option<&CancelToken>,
) -> Result<Observed<T>, CatalogError> {
    let tree = match trees.build(record) {
        Ok(tree) => tree,
        Err(error) => {
            return Ok(failed(
                kind,
                record,
                &error,
                ExportSourceRole::Selected,
                SourceDisposition::ReadFailed,
            ));
        }
    };
    let fields = crate::export_field_walk::inspect(
        &tree,
        record,
        Some(kind),
        ExportSourceRole::Selected,
        cancel,
    )?;
    let mut observed = match build(&tree) {
        Ok(item) => Observed {
            piece: Piece::Item(Box::new(item)),
            source: SourceObservation::new(
                kind,
                record,
                ExportSourceRole::Selected,
                SourceDisposition::Projected,
            ),
            fields: ExportLedger::default(),
        },
        Err(error) => failed(
            kind,
            record,
            error.as_ref(),
            ExportSourceRole::Selected,
            SourceDisposition::ProjectionFailed,
        ),
    };
    observed.fields = fields;
    Ok(observed)
}

fn failed<T>(
    kind: Kind,
    record: &Record<'_>,
    error: &(dyn std::error::Error + 'static),
    role: ExportSourceRole,
    disposition: SourceDisposition,
) -> Observed<T> {
    Observed {
        piece: Piece::Failed(CatalogFailure::of(record, error)),
        source: SourceObservation::new(kind, record, role, disposition)
            .diagnosed(crate::export_source_failure::diagnosis(error)),
        fields: ExportLedger::default(),
    }
}
