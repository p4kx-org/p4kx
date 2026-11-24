// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::read::{self, Query};
use super::rows::{Family, Projector, ReadOptions, WEAPON_SOURCE_TYPES};
use super::weapon::WeaponSourceSnapshot;
use super::weapon_rows::WeaponSources;
use crate::build::catalog_queries::unlocalized_record_query::plan::{Owner, Plan};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    DatasetAvailability, ParsedCatalogQueries,
};
use crate::catalog_query_session::checkpoint;
use crate::catalog_v3::catalog_item_alignment::AlignmentOptions;
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, ExportAccountingError};

/// Final result of visiting a selection of weapon source owners.
#[derive(Debug)]
#[must_use]
pub struct WeaponSourceVisitSummary {
    availability: DatasetAvailability,
    owner_count: usize,
    row_count: usize,
    coverage: CatalogRecordCoverage,
}

impl WeaponSourceVisitSummary {
    /// Returns availability of the canonical weapon source family.
    #[must_use]
    pub const fn availability(&self) -> DatasetAvailability {
        self.availability
    }

    /// Returns the number of owners visited, including snapshots without rows.
    #[must_use]
    pub const fn owner_count(&self) -> usize {
        self.owner_count
    }

    /// Returns the total number of canonical rows passed to the visitor.
    #[must_use]
    pub const fn row_count(&self) -> usize {
        self.row_count
    }

    /// Borrows selection-wide header, body and field projection coverage.
    #[must_use]
    pub const fn coverage(&self) -> &CatalogRecordCoverage {
        &self.coverage
    }
}

/// Visits each readable selected physical owner with its own canonical weapon source snapshot.
/// Callbacks run in Record index order, with the physical index supplied even for an empty snapshot.
///
/// # Errors
///
/// Returns input, Archive-change, cancellation or visitor errors. Stage callback results until
/// the traversal succeeds if a failure must leave the destination unchanged.
pub fn visit_weapon_sources<E: From<CatalogError>>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    visit: impl for<'source> FnMut(u32, WeaponSourceSnapshot<'source>) -> Result<(), E>,
) -> Result<WeaponSourceVisitSummary, E> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .visit_weapon_sources(selection, cancel, visit)
}

/// Visits selected weapon sources from an already decoded DataForge Entry.
///
/// # Errors
///
/// Returns query or visitor errors, including origin mismatch for a foreign Entry.
pub fn visit_weapon_sources_from<E: From<CatalogError>>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    visit: impl for<'source> FnMut(u32, WeaponSourceSnapshot<'source>) -> Result<(), E>,
) -> Result<WeaponSourceVisitSummary, E> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .visit_weapon_sources(selection, cancel, visit)
}

impl CatalogQuerySession<'_> {
    /// Visits selected physical weapon owners through this Archive session.
    ///
    /// # Errors
    ///
    /// Returns query or visitor errors.
    pub fn visit_weapon_sources<E: From<CatalogError>>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        visit: impl for<'source> FnMut(u32, WeaponSourceSnapshot<'source>) -> Result<(), E>,
    ) -> Result<WeaponSourceVisitSummary, E> {
        self.parsed(cancel)?
            .visit_weapon_sources(selection, cancel, visit)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Visits selected physical weapon owners through this parsed DataForge view.
    ///
    /// # Errors
    ///
    /// Returns query or visitor errors.
    pub fn visit_weapon_sources<E: From<CatalogError>>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        visit: impl for<'source> FnMut(u32, WeaponSourceSnapshot<'source>) -> Result<(), E>,
    ) -> Result<WeaponSourceVisitSummary, E> {
        self.project_fallible(cancel, |archive, entry, forge| {
            Query {
                archive,
                entry,
                forge,
                selection,
                languages: &[],
                cancel,
            }
            .visit_weapon_sources(visit)
        })
    }
}

impl Query<'_> {
    fn visit_weapon_sources<E: From<CatalogError>>(
        self,
        mut visit: impl for<'source> FnMut(u32, WeaponSourceSnapshot<'source>) -> Result<(), E>,
    ) -> Result<WeaponSourceVisitSummary, E> {
        let sources = TextSources::read_languages(self.archive, &[])?;
        let texts = Texts::parse(&sources)?;
        let options = ReadOptions {
            alignment: AlignmentOptions::default(),
            item_filter: None,
        };
        let mut owner_count = 0_usize;
        let mut row_count = 0_usize;
        let coverage = Plan::visit_items::<E>(
            self.forge,
            match self.selection {
                CatalogRecordSelection::RecordIndex(_) => None,
                _ => Some(WEAPON_SOURCE_TYPES),
            },
            self.selection,
            self.cancel,
            None,
            |owner, aggregate| {
                let record_index = owner.record.index();
                self.visit_projected_owner(owner, &sources, &texts, &options, |snapshot| {
                    owner_count = owner_count.checked_add(1).ok_or_else(counter_overflow)?;
                    row_count = row_count
                        .checked_add(snapshot.rows().len())
                        .ok_or_else(counter_overflow)?;
                    aggregate
                        .incomplete_fields
                        .extend_from_slice(&snapshot.coverage().incomplete_fields);
                    visit(record_index, snapshot)
                })?;
                checkpoint(self.cancel)?;
                Ok(())
            },
        )?;
        Ok(WeaponSourceVisitSummary {
            availability: Family::WeaponSources.availability(self.forge, row_count),
            owner_count,
            row_count,
            coverage,
        })
    }
    fn visit_projected_owner<E: From<CatalogError>>(
        &self,
        owner: Owner<'_>,
        sources: &TextSources,
        texts: &Texts<'_>,
        options: &ReadOptions<'_>,
        visit: impl for<'item> FnOnce(WeaponSourceSnapshot<'item>) -> Result<(), E>,
    ) -> Result<(), E> {
        let origins = CatalogOrigins::new(self.archive, self.entry, sources)?;
        let input = OriginalCatalogInput::new(self.forge, &origins);
        let registry = CatalogTextRegistry::default();
        let projection =
            CatalogProjection::new(texts, &input, &registry).for_kind(Family::WeaponSources.kind());
        let mut coverage = CatalogRecordCoverage::default();
        let mut rows = read::project_owner::<WeaponSources>(
            self.forge,
            &owner,
            &projection,
            options,
            &mut coverage,
            self.cancel,
        )?;
        drop(owner);
        WeaponSources::finish(&input, &mut rows, &mut coverage, self.cancel)?;
        let names = projection.finish_rows()?;
        let source_inventory = origins.uses.finish_with(|| checkpoint(self.cancel))?;
        checkpoint(self.cancel)?;
        visit(WeaponSourceSnapshot {
            build: self.archive.build().cloned(),
            availability: Family::WeaponSources.availability(self.forge, rows.len()),
            rows,
            public_names: names.public_names,
            game_texts: names.game_texts,
            requested_languages: Vec::new(),
            available_languages: Vec::new(),
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
