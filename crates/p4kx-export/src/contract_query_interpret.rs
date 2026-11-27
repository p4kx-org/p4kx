// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Record, RecordTree};

use super::ContractQueryCoverage;
use super::dependencies::{Dependencies, Inputs};
use super::read::{Projected, Transaction, take_observed};
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_v3::tag_catalog::{self, CapturedTags, TagConsumers};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::ExportLedger;
use crate::mission_entry_source::MissionEntryContext;
use crate::mission_index::{MissionIndex, read::CachedTree};
use crate::mission_places::Places;
use crate::missions::record_read::{
    ContractAttempt, GeneratorFacets, ObservedPiece, Piece, try_project_contracts_tree,
};
use crate::record_observation::RecordObservation;
use crate::spawn_pools::SpawnPools;
use crate::{CatalogError, CatalogReadFailure, ExportAccountingError, Kind};

pub(super) struct Interpreter<'context, 'source> {
    forge: &'source DataForge<'source>,
    projection: &'context CatalogProjection<'context, 'source>,
    tags: CapturedTags<'context, 'source>,
    index: MissionIndex<'source>,
    dependencies: Dependencies<'source>,
    facets: GeneratorFacets,
    cancel: Option<&'context CancelToken>,
}

impl<'context, 'source> Interpreter<'context, 'source> {
    pub(super) fn new(
        forge: &'source DataForge<'source>,
        projection: &'context CatalogProjection<'context, 'source>,
        cancel: Option<&'context CancelToken>,
        facets: GeneratorFacets,
    ) -> Result<Self, CatalogError> {
        let captured = tag_catalog::read(
            forge,
            projection.original(),
            cancel,
            TagConsumers {
                contracts: true,
                paints: false,
            },
        )?;
        let dependencies_tags = captured
            .dependencies
            .ok_or(ExportAccountingError::ConflictingTagTrace)?;
        let mut index = MissionIndex::read_cached_records(
            projection.original(),
            &captured.context,
            dependencies_tags,
            std::iter::empty::<(Record<'source>, CachedTree<'_, 'source>)>(),
            cancel,
        )?;
        let dependencies = Dependencies::new(forge, cancel)?;
        index.set_query_dependencies(dependencies.prepared.clone());
        Ok(Self {
            forge,
            projection,
            tags: captured.context,
            index,
            dependencies,
            facets,
            cancel,
        })
    }

    pub(super) fn generator(
        &mut self,
        tree: &RecordTree<'source>,
        selected: bool,
        transaction: &mut Transaction<'source>,
        output: &mut Projected<'source>,
        coverage: &mut ContractQueryCoverage<'source>,
    ) -> Result<(), CatalogError> {
        let role = if selected {
            ExportSourceRole::Selected
        } else {
            ExportSourceRole::Dependency
        };
        crate::catalog_query_session::checkpoint(self.cancel)?;
        let record = self
            .forge
            .record(tree.record_index())
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let admission = RecordObservation::capture(self.projection.original(), &record)?;
        loop {
            self.dependencies.prepared.begin_attempt()?;
            let registry = CatalogTextRegistry::default();
            let projection = self
                .projection
                .with_text_registry(&registry)
                .for_kind(Kind::Contracts);
            let places = self.dependencies.places();
            match self.attempt(&admission, tree, role, &projection, &places)? {
                ContractAttempt::Dependency(key) => {
                    let place_registry = CatalogTextRegistry::default();
                    let place_projection = CatalogProjection::new(
                        self.projection.legacy_texts(),
                        self.projection.original(),
                        &place_registry,
                    );
                    let inputs = Inputs {
                        original: self.projection.original(),
                        tags: &self.tags,
                        places: &place_projection,
                        cancel: self.cancel,
                    };
                    self.dependencies
                        .prepare(key, &inputs, &mut self.index, coverage)?;
                    crate::catalog_query_session::checkpoint(self.cancel)?;
                }
                ContractAttempt::Complete(observed) => {
                    return self.complete(
                        *observed,
                        &projection,
                        selected,
                        transaction,
                        output,
                        coverage,
                    );
                }
            }
        }
    }

    fn attempt(
        &self,
        admission: &RecordObservation<'_, 'source>,
        tree: &RecordTree<'source>,
        role: ExportSourceRole,
        projection: &CatalogProjection<'_, 'source>,
        places: &Places<'_, 'source>,
    ) -> Result<ContractAttempt<'source>, CatalogError> {
        let context = MissionEntryContext {
            index: &self.index,
            projection,
            places,
            cancel: self.cancel,
        };
        try_project_contracts_tree(
            self.projection.original(),
            admission,
            tree,
            &context,
            role,
            self.cancel,
            self.facets,
        )
    }

    fn complete(
        &self,
        observed: ObservedPiece<'source>,
        staged: &CatalogProjection<'_, 'source>,
        selected: bool,
        transaction: &mut Transaction<'source>,
        output: &mut Projected<'source>,
        coverage: &mut ContractQueryCoverage<'source>,
    ) -> Result<(), CatalogError> {
        crate::catalog_query_session::checkpoint(self.cancel)?;
        let publish_texts = selected && matches!(&observed.piece, Piece::Contracts(_));
        take_observed(observed, selected, transaction, output, coverage)?;
        if publish_texts {
            let consulted = self.dependencies.prepared.consulted()?;
            self.dependencies
                .publish_place_names(&consulted, self.projection, self.cancel)?;
            self.projection.commit_text_registrations(staged)?;
        }
        Ok(())
    }

    pub(super) fn finish(
        mut self,
    ) -> (
        SpawnPools<'source>,
        Vec<CatalogReadFailure<'source>>,
        ExportLedger,
    ) {
        self.index.take_template_results()
    }
}
