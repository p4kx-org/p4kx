// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, Filter, Record, TreeBuilder};
use rayon::prelude::*;

use super::{Scan, selects_group};
use crate::entity_selection::{Selected, select};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition, SourceObservation};
use crate::resource_group::{Groups, Listing, listing};
use crate::{CatalogError, CatalogFailure, Kind};

pub(super) struct GroupRead {
    pub(super) groups: Groups,
    pub(super) failures: Vec<CatalogFailure>,
    pub(super) ledger: ExportLedger,
}

struct GroupPiece {
    complete: bool,
    listing: Option<Listing>,
    failure: Option<CatalogFailure>,
    ledger: ExportLedger,
}

impl<'f> Scan<'_, '_, 'f> {
    /// Reads each group input once and captures its actual dependency read.
    pub(super) fn groups(
        records: &[Record<'f>],
        filter: Option<&Filter>,
        cancel: Option<&CancelToken>,
    ) -> Result<GroupRead, CatalogError> {
        let listed: Vec<GroupPiece> = records
            .par_iter()
            .map_init(TreeBuilder::new, |trees, record| {
                read_group(trees, record, filter, cancel)
            })
            .filter_map(Result::transpose)
            .collect::<Result<_, _>>()?;
        let mut read = GroupRead {
            groups: Groups::default(),
            failures: Vec::new(),
            ledger: ExportLedger::default(),
        };
        for piece in listed {
            if !piece.complete {
                read.groups.mark_incomplete();
            }
            if let Some(listing) = piece.listing {
                read.groups.add(listing);
            }
            read.failures.extend(piece.failure);
            read.ledger.merge(piece.ledger);
        }
        Ok(read)
    }
}

fn read_group<'f>(
    trees: &mut TreeBuilder<'f>,
    record: &Record<'f>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<Option<GroupPiece>, CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    match select(record, None) {
        Ok(Some(Selected::Entity(Kind::ResourceGroups))) => {}
        Ok(_) => return Ok(None),
        Err(_) => {
            if matches!(select(record, filter), Ok(None)) {
                return Ok(None);
            }
            return Ok(Some(GroupPiece {
                complete: false,
                listing: None,
                failure: None,
                ledger: ExportLedger::default(),
            }));
        }
    }
    let mut ledger = ExportLedger::default();
    let (listing, failure, disposition) =
        read_group_tree(trees, record, filter, cancel, &mut ledger)?;
    for kind in [Kind::ResourceGroups, Kind::Commodities, Kind::ResourceTypes] {
        ledger.observe(SourceObservation::new(
            kind,
            record,
            ExportSourceRole::Dependency,
            disposition,
        ));
    }
    let complete = listing.is_some() || !selects_group(record, filter);
    Ok(Some(GroupPiece {
        complete,
        listing,
        failure,
        ledger,
    }))
}

fn read_group_tree<'f>(
    trees: &mut TreeBuilder<'f>,
    record: &Record<'f>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    ledger: &mut ExportLedger,
) -> Result<(Option<Listing>, Option<CatalogFailure>, SourceDisposition), CatalogError> {
    Ok(match trees.build(record) {
        Ok(tree) => {
            ledger.merge(crate::export_field_walk::inspect(
                &tree,
                record,
                Some(Kind::ResourceGroups),
                ExportSourceRole::Dependency,
                cancel,
            )?);
            (
                selects_group(record, filter).then(|| listing(&tree)),
                None,
                SourceDisposition::Projected,
            )
        }
        Err(error) => (
            None,
            (!selects_group(record, filter)).then(|| CatalogFailure::of(record, &error)),
            SourceDisposition::ReadFailed,
        ),
    })
}
