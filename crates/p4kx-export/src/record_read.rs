// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter, Record, RecordTree, TreeBuilder};
use rayon::prelude::*;

use crate::guid_text::{CatalogGuid, sort_by_guid};
use crate::selection::{Kind, is_kind_leftover};
use crate::{CatalogError, CatalogFailure};

/// The entities of one kind of a DataForge and what was left out.
pub(crate) struct KindRead<T> {
    pub(crate) items: Vec<T>,
    pub(crate) skipped: usize,
    pub(crate) failures: Vec<CatalogFailure>,
}

/// A Record that cannot be read: the error of the DataForge or of an identifier of a description.
pub(crate) type ReadError = Box<dyn std::error::Error + Send + Sync>;

enum Piece<T> {
    Item(Box<T>),
    Skipped,
    Failed(CatalogFailure),
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
    let pieces: Vec<Piece<T>> = records
        .par_iter()
        .map_init(TreeBuilder::new, |trees, record| {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            Ok(piece(kind, trees, record, &build).unwrap_or_else(|source| {
                Some(Piece::Failed(CatalogFailure::of(record, source.as_ref())))
            }))
        })
        .filter_map(Result::transpose)
        .collect::<Result<_, _>>()?;
    let mut read = KindRead {
        items: Vec::new(),
        skipped: 0,
        failures: Vec::new(),
    };
    for piece in pieces {
        match piece {
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
) -> Result<Option<Piece<T>>, ReadError> {
    let rule = kind.records();
    let path = record.record_path()?;
    if record.type_name()? != rule.type_name {
        return Ok(None);
    }
    if is_kind_leftover(kind, path, record.name()?) {
        return Ok(Some(Piece::Skipped));
    }
    let tree = trees.build(record)?;
    build(&tree).map(|item| Some(Piece::Item(Box::new(item))))
}
