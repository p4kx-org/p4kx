// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::OnceCell;

use p4kx_extraction::{CancelToken, Language};

use super::read::{self, Query};
use super::rows::{Projector, ReadOptions};
use super::snapshot::EquipmentSnapshot;
use crate::build::catalog_queries::unlocalized_record_query::plan::{Owner, Plan};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_queries::record_selection::RecordVisit;
use crate::catalog_queries::{
    CatalogRecordCoverage, CatalogRecordSelection, DatasetAvailability, ParsedCatalogQueries,
};
use crate::catalog_query_session::checkpoint;
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, ExportAccountingError, Kind};

/// Outcome of visiting a selected physical equipment family.
#[derive(Debug)]
#[must_use]
pub struct EquipmentVisitSummary {
    availability: DatasetAvailability,
    owner_count: usize,
    row_count: usize,
    coverage: CatalogRecordCoverage,
    missing_record_indices: Vec<u32>,
}

impl EquipmentVisitSummary {
    /// Returns the availability of the authored family.
    #[must_use]
    pub const fn availability(&self) -> DatasetAvailability {
        self.availability
    }

    /// Returns the number of readable owner candidates visited, including empty snapshots.
    #[must_use]
    pub const fn owner_count(&self) -> usize {
        self.owner_count
    }

    /// Returns the number of physical component rows passed to the visitor.
    #[must_use]
    pub const fn row_count(&self) -> usize {
        self.row_count
    }

    /// Borrows the requested batch indices absent from the original DataForge.
    #[must_use]
    pub fn missing_record_indices(&self) -> &[u32] {
        &self.missing_record_indices
    }

    /// Borrows the selection-wide Record read and projection coverage.
    #[must_use]
    pub const fn coverage(&self) -> &CatalogRecordCoverage {
        &self.coverage
    }
}

#[derive(Clone, Copy)]
pub(super) enum EntityScope<'selection> {
    All,
    Items(Option<&'selection [u32]>),
    Kinds(&'static [Kind]),
    Components(&'static [&'static str]),
}

impl EntityScope<'_> {
    fn visit<'archive, E: From<CatalogError>>(
        self,
        query: &Query<'archive>,
        missing_record_indices: &mut Vec<u32>,
        visit: impl FnMut(Owner<'archive>, &mut CatalogRecordCoverage) -> Result<(), E>,
    ) -> Result<CatalogRecordCoverage, E> {
        match self {
            Self::All => {
                Plan::visit_entities(query.forge, query.selection, None, query.cancel, visit)
            }
            Self::Items(indices) => {
                let selection = indices.map_or_else(
                    || query.selection.into(),
                    |indices| RecordVisit::Indices {
                        indices,
                        missing: missing_record_indices,
                    },
                );
                Plan::visit_items(query.forge, None, selection, query.cancel, None, visit)
            }
            Self::Kinds(kinds) => Plan::visit_entities(
                query.forge,
                query.selection,
                Some(kinds),
                query.cancel,
                visit,
            ),
            Self::Components(types) => Plan::visit_entity_components(
                query.forge,
                types,
                query.selection,
                query.cancel,
                visit,
            ),
        }
    }
}

impl ParsedCatalogQueries<'_> {
    pub(super) fn visit_equipment_owners<P: Projector, E: From<CatalogError>>(
        &self,
        selection: CatalogRecordSelection,
        scope: EntityScope<'_>,
        cancel: Option<&CancelToken>,
        visit: impl for<'source> FnMut(
            u32,
            EquipmentSnapshot<'source, P::Row<'source>>,
        ) -> Result<(), E>,
    ) -> Result<EquipmentVisitSummary, E> {
        self.project_fallible(cancel, |archive, entry, forge| {
            Query {
                archive,
                entry,
                forge,
                selection,
                languages: self.languages(),
                cancel,
            }
            .visit_equipment_owners::<P, E>(scope, visit)
        })
    }
}

impl Query<'_> {
    fn visit_equipment_owners<P: Projector, E: From<CatalogError>>(
        self,
        scope: EntityScope<'_>,
        mut inspect: impl for<'source> FnMut(
            u32,
            EquipmentSnapshot<'source, P::Row<'source>>,
        ) -> Result<(), E>,
    ) -> Result<EquipmentVisitSummary, E> {
        let family = P::FAMILY;
        let declared_availability = family.availability(self.forge, 0);
        let options = self.read_options(family, None)?;
        let languages = self.requested(family);
        let source_cache = OnceCell::new();
        let text_cache = OnceCell::new();
        let mut owner_count = 0_usize;
        let mut row_count = 0_usize;
        let mut missing_record_indices = Vec::new();
        let coverage =
            scope.visit::<E>(&self, &mut missing_record_indices, |owner, aggregate| {
                checkpoint(self.cancel)?;
                let sources = if let Some(sources) = source_cache.get() {
                    sources
                } else {
                    let sources = TextSources::read_languages(self.archive, languages)?;
                    source_cache.get_or_init(|| sources)
                };
                checkpoint(self.cancel)?;
                let texts = if let Some(texts) = text_cache.get() {
                    texts
                } else {
                    let texts = Texts::parse(sources)?;
                    text_cache.get_or_init(|| texts)
                };
                let index = owner.record.index();
                let inputs = OwnerInputs {
                    sources,
                    texts,
                    options: &options,
                    languages,
                    declared_availability,
                };
                self.visit_equipment_owner::<P, E>(owner, &inputs, |snapshot| {
                    owner_count = owner_count.checked_add(1).ok_or_else(counter_overflow)?;
                    row_count = row_count
                        .checked_add(snapshot.rows().len())
                        .ok_or_else(counter_overflow)?;
                    aggregate
                        .incomplete_fields
                        .extend_from_slice(&snapshot.coverage().incomplete_fields);
                    inspect(index, snapshot)
                })?;
                checkpoint(self.cancel)?;
                Ok(())
            })?;
        Ok(EquipmentVisitSummary {
            availability: observed_availability(declared_availability, row_count),
            owner_count,
            row_count,
            coverage,
            missing_record_indices,
        })
    }

    fn visit_equipment_owner<P: Projector, E: From<CatalogError>>(
        &self,
        owner: Owner<'_>,
        inputs: &OwnerInputs<'_, '_>,
        inspect: impl for<'source> FnOnce(EquipmentSnapshot<'source, P::Row<'source>>) -> Result<(), E>,
    ) -> Result<(), E> {
        let OwnerInputs {
            sources,
            texts,
            options,
            languages,
            declared_availability,
        } = *inputs;
        let origins = self.entry.with_texts(self.archive, sources)?;
        let input = OriginalCatalogInput::new(self.forge, &origins);
        let registry = CatalogTextRegistry::default();
        let family = P::FAMILY;
        let projection = CatalogProjection::new(texts, &input, &registry).for_kind(family.kind());
        let mut coverage = CatalogRecordCoverage::default();
        let mut rows = read::project_owner::<P>(
            self.forge,
            &owner,
            &projection,
            options,
            &mut coverage,
            self.cancel,
        )?;
        drop(owner);
        P::finish(&input, &mut rows, &mut coverage, self.cancel)?;
        let names = projection.finish_rows()?;
        let source_inventory = origins.uses.finish_with(|| checkpoint(self.cancel))?;
        checkpoint(self.cancel)?;
        inspect(EquipmentSnapshot {
            build: self.archive.build().cloned(),
            availability: observed_availability(declared_availability, rows.len()),
            rows,
            public_names: names.public_names,
            game_texts: names.game_texts,
            requested_languages: languages.to_vec(),
            available_languages: languages
                .iter()
                .copied()
                .filter(|language| sources.entry(*language).is_some())
                .collect(),
            localization_notes: texts.notes().to_vec(),
            source_inventory,
            coverage,
            dependency_coverage: (),
        })
    }
}

struct OwnerInputs<'scope, 'source> {
    sources: &'scope TextSources,
    texts: &'scope Texts<'source>,
    options: &'scope ReadOptions<'source>,
    languages: &'scope [Language],
    declared_availability: DatasetAvailability,
}

const fn observed_availability(declared: DatasetAvailability, rows: usize) -> DatasetAvailability {
    if rows == 0 {
        declared
    } else {
        DatasetAvailability::Available
    }
}

fn counter_overflow() -> CatalogError {
    ExportAccountingError::CounterOverflow.into()
}
