// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{
    CancelToken, CanonicalReadObserver, DataForge, Record, RecordTree, TreeBuilder,
};

use crate::catalog_queries::unlocalized_record_query;
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session::checkpoint;
use crate::{CatalogError, Kind};

pub(in crate::build::catalog_queries) struct Owner<'source> {
    pub(in crate::build::catalog_queries) record: Record<'source>,
    pub(in crate::build::catalog_queries) tree: RecordTree<'source>,
}

pub(in crate::build::catalog_queries) struct Plan<'source> {
    pub(in crate::build::catalog_queries) owners: Vec<Owner<'source>>,
    pub(in crate::build::catalog_queries) coverage: CatalogRecordCoverage,
}

impl<'source> Plan<'source> {
    pub(in crate::build::catalog_queries) fn read(
        forge: &'source DataForge<'source>,
        kind: Kind,
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
            if !unlocalized_record_query::selected(record, kind, &mut result.coverage) {
                return Ok(());
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
        Ok(result)
    }
}
