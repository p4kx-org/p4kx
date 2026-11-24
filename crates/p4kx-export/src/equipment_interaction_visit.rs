// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::OnceCell;

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::super::read::{self, Query};
use super::super::rows::{Projector, ReadOptions};
use super::{InteractionSnapshot, Interactions};
use crate::build::catalog_queries::unlocalized_record_query::plan::{Owner, Plan};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    DatasetAvailability, ParsedCatalogQueries,
};
use crate::catalog_query_session::checkpoint;
use crate::catalog_v3::catalog_interactions::SOURCE_TYPES;
use crate::catalog_v3::catalog_item_alignment::AlignmentOptions;
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, ExportAccountingError};

/// Outcome of visiting the selected original interaction components.
#[derive(Debug)]
#[must_use]
pub struct InteractionVisitSummary {
    availability: DatasetAvailability,
    owner_count: usize,
    row_count: usize,
    coverage: CatalogRecordCoverage,
}

impl InteractionVisitSummary {
    /// Returns the availability of the authored interaction family.
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

    /// Borrows the selection-wide Record read and projection coverage.
    #[must_use]
    pub const fn coverage(&self) -> &CatalogRecordCoverage {
        &self.coverage
    }
}

/// Visits readable interaction candidates in physical Record order with owner-local sources.
/// Stage consumer changes until the visit succeeds and its coverage meets the caller's requirements.
///
/// # Errors
///
/// Returns input, Archive-change, cancellation or visitor errors.
pub fn visit_interactions<E: From<CatalogError>>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    visit: impl for<'source> FnMut(u32, &InteractionSnapshot<'source>) -> Result<(), E>,
) -> Result<InteractionVisitSummary, E> {
    CatalogQuerySession::open(archive, languages, cancel)?
        .visit_interactions(selection, cancel, visit)
}

/// Visits interactions from the same Archive's previously decoded DataForge Entry.
///
/// # Errors
///
/// Returns query or visitor errors, including a source-origin mismatch for a foreign Entry.
pub fn visit_interactions_from<E: From<CatalogError>>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    visit: impl for<'source> FnMut(u32, &InteractionSnapshot<'source>) -> Result<(), E>,
) -> Result<InteractionVisitSummary, E> {
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .visit_interactions(selection, cancel, visit)
}

impl CatalogQuerySession<'_> {
    /// Visits selected physical interaction owners under this session's locale policy.
    ///
    /// # Errors
    ///
    /// Returns query or visitor errors.
    pub fn visit_interactions<E: From<CatalogError>>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        visit: impl for<'source> FnMut(u32, &InteractionSnapshot<'source>) -> Result<(), E>,
    ) -> Result<InteractionVisitSummary, E> {
        self.parsed(cancel)?
            .visit_interactions(selection, cancel, visit)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Visits selected physical interaction owners using the parsed original DataForge.
    ///
    /// # Errors
    ///
    /// Returns query or visitor errors.
    pub fn visit_interactions<E: From<CatalogError>>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        visit: impl for<'source> FnMut(u32, &InteractionSnapshot<'source>) -> Result<(), E>,
    ) -> Result<InteractionVisitSummary, E> {
        self.project_fallible(cancel, |archive, entry, forge| {
            Query {
                archive,
                entry,
                forge,
                selection,
                languages: self.languages(),
                cancel,
            }
            .visit_interactions(visit)
        })
    }
}

impl Query<'_> {
    fn visit_interactions<E: From<CatalogError>>(
        self,
        mut visit: impl for<'source> FnMut(u32, &InteractionSnapshot<'source>) -> Result<(), E>,
    ) -> Result<InteractionVisitSummary, E> {
        let source_cache = OnceCell::new();
        let text_cache = OnceCell::new();
        let mut owner_count = 0_usize;
        let mut row_count = 0_usize;
        let coverage = Plan::visit_entity_components::<E>(
            self.forge,
            SOURCE_TYPES,
            self.selection,
            self.cancel,
            |owner, aggregate| {
                checkpoint(self.cancel)?;
                let sources = if let Some(sources) = source_cache.get() {
                    sources
                } else {
                    let sources = TextSources::read_languages(self.archive, self.languages)?;
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
                self.visit_interaction_owner(owner, sources, texts, |snapshot| {
                    owner_count = owner_count.checked_add(1).ok_or_else(counter_overflow)?;
                    row_count = row_count
                        .checked_add(snapshot.rows().len())
                        .ok_or_else(counter_overflow)?;
                    aggregate
                        .incomplete_fields
                        .extend_from_slice(&snapshot.coverage().incomplete_fields);
                    visit(index, snapshot)
                })?;
                checkpoint(self.cancel)?;
                Ok(())
            },
        )?;
        Ok(InteractionVisitSummary {
            availability: Interactions::FAMILY.availability(self.forge, row_count),
            owner_count,
            row_count,
            coverage,
        })
    }

    fn visit_interaction_owner<E: From<CatalogError>>(
        &self,
        owner: Owner<'_>,
        sources: &TextSources,
        texts: &Texts<'_>,
        inspect: impl for<'source> FnOnce(&InteractionSnapshot<'source>) -> Result<(), E>,
    ) -> Result<(), E> {
        let origins = CatalogOrigins::new(self.archive, self.entry, sources)?;
        let input = OriginalCatalogInput::new(self.forge, &origins);
        let registry = CatalogTextRegistry::default();
        let family = Interactions::FAMILY;
        let projection = CatalogProjection::new(texts, &input, &registry).for_kind(family.kind());
        let options = ReadOptions {
            alignment: AlignmentOptions::default(),
            item_filter: None,
        };
        let mut coverage = CatalogRecordCoverage::default();
        let mut rows = read::project_owner::<Interactions>(
            self.forge,
            &owner,
            &projection,
            &options,
            &mut coverage,
            self.cancel,
        )?;
        drop(owner);
        Interactions::finish(&input, &mut rows, &mut coverage, self.cancel)?;
        let names = projection.finish_rows()?;
        let source_inventory = origins.uses.finish_with(|| checkpoint(self.cancel))?;
        checkpoint(self.cancel)?;
        inspect(&InteractionSnapshot {
            build: self.archive.build().cloned(),
            availability: family.availability(self.forge, rows.len()),
            rows,
            public_names: names.public_names,
            game_texts: names.game_texts,
            requested_languages: self.languages.to_vec(),
            available_languages: self
                .languages
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

fn counter_overflow() -> CatalogError {
    ExportAccountingError::CounterOverflow.into()
}
