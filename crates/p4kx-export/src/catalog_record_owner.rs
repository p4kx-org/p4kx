// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, DataForgeError, Record, TreeBuilder};

use super::Owner;
use crate::CatalogError;
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session::checkpoint;
use crate::guid_text::{CatalogGuid, sort_by_guid};
use crate::selection::KindRule;

pub(in crate::build::catalog_queries) struct OrderedOwnerPlan<'source> {
    records: Vec<Record<'source>>,
    coverage: CatalogRecordCoverage,
}

impl<'source> OrderedOwnerPlan<'source> {
    pub(in crate::build::catalog_queries) fn select(
        forge: &'source DataForge<'source>,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        mut selected: impl FnMut(&Record<'source>, &mut CatalogRecordCoverage) -> bool,
    ) -> Result<Self, CatalogError> {
        let mut records = Vec::new();
        let mut coverage = CatalogRecordCoverage::default();
        selection.for_each(forge, cancel, |record| {
            checkpoint(cancel)?;
            if selected(record, &mut coverage) {
                records.push(*record);
            }
            Ok(())
        })?;
        sort_by_guid(&mut records, |record| CatalogGuid(record.guid()));
        Ok(Self { records, coverage })
    }

    pub(in crate::build::catalog_queries) fn project<Row>(
        self,
        cancel: Option<&CancelToken>,
        mut project: impl FnMut(Owner<'source>, &mut CatalogRecordCoverage) -> Result<Row, CatalogError>,
    ) -> Result<(Vec<Row>, CatalogRecordCoverage), CatalogError> {
        let mut rows = Vec::with_capacity(self.records.len());
        let coverage = self.visit::<CatalogError>(cancel, |owner, coverage| {
            rows.push(project(owner, coverage)?);
            Ok(())
        })?;
        Ok((rows, coverage))
    }

    pub(in crate::build::catalog_queries) fn visit<E: From<CatalogError>>(
        self,
        cancel: Option<&CancelToken>,
        mut visit: impl FnMut(Owner<'source>, &mut CatalogRecordCoverage) -> Result<(), E>,
    ) -> Result<CatalogRecordCoverage, E> {
        let Self {
            records,
            mut coverage,
        } = self;
        let mut trees = TreeBuilder::new();
        for record in records {
            checkpoint(cancel)?;
            if let Some(owner) = read_owner(&mut trees, &record, &mut coverage)? {
                checkpoint(cancel)?;
                visit(owner, &mut coverage)?;
            }
        }
        // Coverage follows physical Record order, independent of row ordering.
        coverage.unreadable_bodies.sort_unstable();
        Ok(coverage)
    }
}

pub(in crate::build::catalog_queries) fn rule_allows(
    record: &Record<'_>,
    rule: KindRule,
    coverage: &mut CatalogRecordCoverage,
) -> bool {
    let path = record.record_path();
    if path.as_ref().is_ok_and(|path| !rule.matches_path(path)) {
        return false;
    }
    match record.type_name() {
        Ok(source_type) if source_type != rule.type_name => false,
        Ok(_) if path.is_ok() => true,
        _ => {
            coverage.unreadable_headers.push(record.index());
            false
        }
    }
}

pub(in crate::build::catalog_queries) fn read_owner<'source>(
    trees: &mut TreeBuilder<'source>,
    record: &Record<'source>,
    coverage: &mut CatalogRecordCoverage,
) -> Result<Option<Owner<'source>>, CatalogError> {
    match trees.build(record) {
        Ok(tree) => Ok(Some(Owner {
            record: *record,
            tree,
        })),
        Err(DataForgeError::ObservationStopped) => Err(CatalogError::Cancelled),
        Err(_) => {
            coverage.unreadable_bodies.push(record.index());
            Ok(None)
        }
    }
}
