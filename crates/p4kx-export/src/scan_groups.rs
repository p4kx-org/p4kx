// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForgeError, Filter, Record, RecordTree, TreeBuilder};
use rayon::prelude::*;

use super::{Scan, selects_group};
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_v3::resource_group_catalog::{self, ResourceGroupMembers};
use crate::entity_selection::{Selected, select};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition, SourceObservation};
use crate::resource_group::{Groups, Listing, ParentSource, listing};
use crate::{CatalogError, CatalogFailure, Kind};

pub(super) struct GroupRead {
    pub(super) groups: Groups,
    pub(super) members: Vec<ResourceGroupMembers>,
    pub(super) failures: Vec<CatalogFailure>,
    pub(super) ledger: ExportLedger,
}

struct GroupPiece {
    complete: bool,
    listing: Option<Listing>,
    members: Option<ResourceGroupMembers>,
    parent_failure: Option<ParentSource>,
    failure: Option<CatalogFailure>,
    disposition: SourceDisposition,
    ledger: ExportLedger,
}

impl<'f> Scan<'_, '_, 'f> {
    /// Reads each group input and freezes its authored arrays before selected-map construction.
    pub(super) fn groups(
        records: &[Record<'f>],
        filter: Option<&Filter>,
        origins: &CatalogOrigins,
        cancel: Option<&CancelToken>,
    ) -> Result<GroupRead, CatalogError> {
        let listed: Vec<GroupPiece> = records
            .par_iter()
            .map_init(TreeBuilder::new, |trees, record| {
                read_group(trees, record, filter, origins, cancel)
            })
            .filter_map(Result::transpose)
            .collect::<Result<_, _>>()?;
        let mut read = GroupRead {
            groups: Groups::default(),
            members: Vec::new(),
            failures: Vec::new(),
            ledger: ExportLedger::default(),
        };
        for piece in listed {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            if !piece.complete {
                read.groups.mark_incomplete();
            }
            if let Some(failure) = piece.parent_failure {
                read.groups.parent_failure(failure);
            }
            if let Some(listing) = piece.listing {
                read.groups.add(listing, cancel)?;
            }
            read.members.extend(piece.members);
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
    origins: &CatalogOrigins,
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
                members: None,
                parent_failure: ParentSource::metadata_failure(record, origins)?,
                failure: None,
                disposition: SourceDisposition::ReadFailed,
                ledger: ExportLedger::default(),
            }));
        }
    }
    let selected = selects_group(record, filter);
    let mut piece = match trees.build(record) {
        Ok(tree) => readable(&tree, record, selected, origins, cancel)?,
        Err(error) => failed_read(record, &error, selected, origins)?,
    };
    for kind in [Kind::ResourceGroups, Kind::Commodities, Kind::ResourceTypes] {
        piece.ledger.observe(SourceObservation::new(
            kind,
            record,
            ExportSourceRole::Dependency,
            piece.disposition,
        ));
    }
    Ok(Some(piece))
}

fn readable(
    tree: &RecordTree<'_>,
    record: &Record<'_>,
    selected: bool,
    origins: &CatalogOrigins,
    cancel: Option<&CancelToken>,
) -> Result<GroupPiece, CatalogError> {
    let mut ledger = crate::export_field_walk::inspect(
        tree,
        record,
        Some(Kind::ResourceGroups),
        ExportSourceRole::Dependency,
        cancel,
    )?;
    let (members, listing) = if selected {
        let (members, notes) = resource_group_catalog::members(tree, origins, cancel)?;
        crate::source_projection_notes::record(
            Kind::ResourceGroups,
            ExportSourceRole::Selected,
            tree,
            notes,
            &mut ledger,
        );
        (Some(members), Some(listing(tree, origins, cancel)?))
    } else {
        (None, None)
    };
    Ok(GroupPiece {
        complete: true,
        listing,
        members,
        parent_failure: None,
        failure: None,
        disposition: SourceDisposition::Projected,
        ledger,
    })
}

fn failed_read(
    record: &Record<'_>,
    error: &DataForgeError,
    selected: bool,
    origins: &CatalogOrigins,
) -> Result<GroupPiece, CatalogError> {
    Ok(GroupPiece {
        complete: !selected,
        listing: None,
        members: None,
        parent_failure: if selected {
            Some(ParentSource::canonical_failure(record, origins)?)
        } else {
            None
        },
        failure: (!selected).then(|| CatalogFailure::of(record, error)),
        disposition: SourceDisposition::ReadFailed,
        ledger: ExportLedger::default(),
    })
}
