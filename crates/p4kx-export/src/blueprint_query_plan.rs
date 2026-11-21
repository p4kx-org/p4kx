// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{
    CancelToken, CanonicalReadObserver, DataForge, Record, RecordTree, TreeBuilder,
};

use crate::CatalogError;
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session::checkpoint;
use crate::selection::{self, BlueprintSelection};

pub(super) struct Owner<'source> {
    pub(super) record: Record<'source>,
    pub(super) tree: RecordTree<'source>,
}

pub(super) struct Plan<'source> {
    pub(super) owners: Vec<Owner<'source>>,
    pub(super) coverage: CatalogRecordCoverage,
}

impl<'source> Plan<'source> {
    pub(super) fn read(
        forge: &'source DataForge<'source>,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        observer: Option<Arc<dyn CanonicalReadObserver>>,
    ) -> Result<Self, CatalogError> {
        let mut result = Self {
            owners: Vec::new(),
            coverage: CatalogRecordCoverage::default(),
        };
        let mut trees = TreeBuilder::new();
        trees.set_read_observer(observer);
        selection.for_each(forge, cancel, |record| {
            match selection::selects_blueprint(record, None) {
                Ok(BlueprintSelection::Outside) => return Ok(()),
                Ok(BlueprintSelection::Leftover) => {
                    result.coverage.skipped_leftovers += 1;
                    return Ok(());
                }
                Err(_) => {
                    result.coverage.unreadable_headers.push(record.index());
                    return Ok(());
                }
                Ok(BlueprintSelection::Kept) => {}
            }
            checkpoint(cancel)?;
            match trees.build(record) {
                Ok(tree) => result.owners.push(Owner {
                    record: *record,
                    tree,
                }),
                Err(_) => result.coverage.unreadable_bodies.push(record.index()),
            }
            Ok(())
        })?;
        result.owners.sort_by_cached_key(|owner| {
            (
                crate::guid_text::CatalogGuid(owner.record.guid()).text(),
                owner.record.index(),
            )
        });
        Ok(result)
    }
}
