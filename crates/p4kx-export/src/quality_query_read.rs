// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{Archive, CancelToken, CanonicalReadObserver, DataForge, TreeBuilder};

use super::rows::Projector;
use super::snapshot::QualityDefinitionSnapshot;
use super::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_query_session::{CatalogQuerySource, checkpoint};
use crate::catalog_read::original::OriginalFlowFields;
use crate::guid_text::CatalogGuid;
use crate::record_observation::RecordObservation;
use crate::template_field_capture::TemplateFieldCapture;

pub(super) struct Query<'query> {
    pub(super) archive: &'query Archive,
    pub(super) entry: CatalogQuerySource<'query>,
    pub(super) forge: &'query DataForge<'query>,
    pub(super) selection: CatalogRecordSelection,
    pub(super) cancel: Option<&'query CancelToken>,
    pub(super) observer: Option<Arc<dyn CanonicalReadObserver>>,
}

impl Query<'_> {
    pub(super) fn run<P: Projector, T>(
        self,
        inspect: impl for<'source> FnOnce(QualityDefinitionSnapshot<P::Row<'source>>) -> T,
    ) -> Result<T, CatalogError> {
        checkpoint(self.cancel)?;
        let origins = self.entry.records(self.archive)?;
        let input = OriginalCatalogInput::new(self.forge, &origins);
        let mut coverage = CatalogRecordCoverage::default();
        let rows = self.read_rows::<P>(&input, &mut coverage)?;
        let source_inventory = origins.uses.finish_with(|| checkpoint(self.cancel))?;
        checkpoint(self.cancel)?;
        Ok(inspect(QualityDefinitionSnapshot {
            build: self.archive.build().cloned(),
            availability: P::FAMILY.availability(self.forge, rows.len()),
            rows,
            source_inventory,
            coverage,
        }))
    }

    fn read_rows<'source, P: Projector>(
        &self,
        input: &OriginalCatalogInput<'_, 'source>,
        coverage: &mut CatalogRecordCoverage,
    ) -> Result<Vec<P::Row<'source>>, CatalogError>
    where
        Self: 'source,
    {
        let mut trees = TreeBuilder::new();
        trees.set_read_observer(self.observer.clone());
        let mut rows = Vec::new();
        self.selection.for_each(self.forge, self.cancel, |record| {
            if !P::FAMILY.selected(record, coverage) {
                return Ok(());
            }
            checkpoint(self.cancel)?;
            let Ok(tree) = trees.build(record) else {
                coverage.unreadable_bodies.push(record.index());
                return Ok(());
            };
            let admission = RecordObservation::capture(input, record)?;
            let scope = input.record_scope(admission.record(), &tree)?;
            let original = OriginalFlowFields::read(&scope, self.cancel)?;
            let capture = TemplateFieldCapture::new(&original, input, &admission)?;
            let found = P::read(&tree, input.origins(), self.cancel, &capture)?;
            if found
                .notes
                .iter()
                .any(|note| note.description().issue.blocks_projection())
            {
                coverage.incomplete_fields.push(record.index());
            }
            rows.push((
                CatalogGuid(record.guid()).text(),
                record.index(),
                found.value,
            ));
            Ok(())
        })?;
        rows.sort_by_key(|(guid, index, _)| (*guid, *index));
        Ok(rows.into_iter().map(|(_, _, row)| row).collect())
    }
}
