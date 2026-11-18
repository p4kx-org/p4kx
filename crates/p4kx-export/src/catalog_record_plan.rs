// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{
    CancelToken, CanonicalReadObserver, DataForge, Record, RecordTree, TreeBuilder,
};

use crate::catalog_queries::unlocalized_record_query;
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session::checkpoint;
use crate::entity_selection::{self, Selected};
use crate::scan::{CRAFTER_FIELD, CRAFTER_STRUCT};
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
        Self::read_with(forge, selection, cancel, observer, |record, _, coverage| {
            Ok(unlocalized_record_query::selected(record, kind, coverage))
        })
    }

    pub(in crate::build::catalog_queries) fn read_entities(
        forge: &'source DataForge<'source>,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        observer: Option<Arc<dyn CanonicalReadObserver>>,
    ) -> Result<Self, CatalogError> {
        let crafter = forge.pointer_probe(CRAFTER_FIELD, CRAFTER_STRUCT);
        Self::read_with(
            forge,
            selection,
            cancel,
            observer,
            |record, trees, coverage| {
                let classified = record.type_name().and_then(|source_type| {
                    if source_type == p4kx_assembly::ENTITY_TYPE {
                        entity_selection::select(record, None)
                    } else {
                        Ok(None)
                    }
                });
                match classified {
                    Err(_) => coverage.unreadable_headers.push(record.index()),
                    Ok(Some(Selected::Leftover(_))) => coverage.skipped_leftovers += 1,
                    Ok(Some(Selected::Entity(kind))) => {
                        if kind != Kind::Crafters {
                            return Ok(true);
                        }
                        match crafter
                            .as_ref()
                            .map_or(Ok(false), |probe| trees.holds(probe, record))
                        {
                            Ok(holds) => return Ok(holds),
                            Err(_) => coverage.unreadable_headers.push(record.index()),
                        }
                    }
                    Ok(None) => {}
                }
                Ok(false)
            },
        )
    }

    pub(in crate::build::catalog_queries) fn read_rule(
        forge: &'source DataForge<'source>,
        rule: crate::selection::KindRule,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        observer: Option<Arc<dyn CanonicalReadObserver>>,
    ) -> Result<Self, CatalogError> {
        Self::read_with(forge, selection, cancel, observer, |record, _, coverage| {
            let path = record.record_path();
            if path.as_ref().is_ok_and(|path| !rule.matches_path(path)) {
                return Ok(false);
            }
            match record.type_name() {
                Ok(source_type) if source_type != rule.type_name => Ok(false),
                Ok(_) if path.is_ok() => Ok(true),
                _ => {
                    coverage.unreadable_headers.push(record.index());
                    Ok(false)
                }
            }
        })
    }

    fn read_with(
        forge: &'source DataForge<'source>,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        observer: Option<Arc<dyn CanonicalReadObserver>>,
        mut selected: impl FnMut(
            &Record<'source>,
            &mut TreeBuilder<'source>,
            &mut CatalogRecordCoverage,
        ) -> Result<bool, CatalogError>,
    ) -> Result<Self, CatalogError> {
        let mut result = Self {
            owners: Vec::new(),
            coverage: CatalogRecordCoverage::default(),
        };
        let mut trees = TreeBuilder::new();
        trees.set_read_observer(observer);
        selection.for_each(forge, cancel, |record| {
            checkpoint(cancel)?;
            if !selected(record, &mut trees, &mut result.coverage)? {
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
