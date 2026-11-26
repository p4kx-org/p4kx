// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{ArchiveFiles, Entities};
use p4kx_extraction::{Archive, CancelToken, DataForge, DataForgeError, Language};

use super::ShipSourceDependencyCoverage;
use super::{ShipSnapshot, ShipSourceSnapshot, complete};
use crate::build::catalog_queries::unlocalized_record_query::plan::{Owner, Plan};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection, DatasetAvailability};
use crate::catalog_query_session::{CatalogQuerySource, checkpoint};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::ExportLedger;
use crate::playability_rules::{Evidence, ShipDependencies};
use crate::record_observation::RecordObservation;
use crate::ship::{ShipSource, source_projection};
use crate::ship_pool::ShipPool;
use crate::texts::{TextSources, Texts};
use crate::variant_use_sets::VariantSourceUseSets;
use crate::{CatalogError, ExportAccountingError, Kind};

#[path = "equipment_ship_owner_groups.rs"]
mod owner_groups;

use owner_groups::OwnerGroups;

pub(super) struct Query<'query> {
    pub(super) archive: &'query Archive,
    pub(super) entry: CatalogQuerySource<'query>,
    pub(super) forge: &'query DataForge<'query>,
    pub(super) selection: CatalogRecordSelection,
    pub(super) languages: &'query [Language],
    pub(super) cancel: Option<&'query CancelToken>,
}

#[derive(Clone, Copy)]
enum Scope {
    Sources,
    Canonical,
}

struct SelectedShips<'source> {
    rows: Vec<ShipSource<'source>>,
    coverage: CatalogRecordCoverage,
    dependencies: ShipSourceDependencyCoverage,
}

impl<'query> Query<'query> {
    pub(super) fn run<T>(
        self,
        inspect: impl FnOnce(ShipSourceSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        checkpoint(self.cancel)?;
        let owners = self.owners()?;
        self.project_plan(owners, Scope::Sources, |snapshot, _| inspect(snapshot))
    }

    pub(super) fn run_plan<T>(
        &self,
        plan: Plan<'query>,
        inspect: impl FnOnce(ShipSourceSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project_plan(
            OwnerGroups::prepared(plan),
            Scope::Sources,
            |snapshot, _| inspect(snapshot),
        )
    }

    pub(super) fn run_ships<T>(
        &self,
        inspect: impl FnOnce(ShipSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        checkpoint(self.cancel)?;
        let owners = self.owners()?;
        self.project_plan(owners, Scope::Canonical, |snapshot, variant_sets| {
            inspect(ShipSnapshot::new(snapshot, variant_sets))
        })
    }

    fn project_plan<T>(
        &self,
        mut groups: OwnerGroups<'query>,
        scope: Scope,
        inspect: impl FnOnce(ShipSourceSnapshot<'_>, VariantSourceUseSets) -> T,
    ) -> Result<T, CatalogError> {
        let mut owners = Vec::new();
        groups.fill(&mut owners, self.cancel)?;
        if owners.is_empty() {
            return self.empty(groups.finish(), |snapshot| {
                inspect(snapshot, VariantSourceUseSets::default())
            });
        }
        let languages = self.languages;
        let sources = TextSources::read_languages(self.archive, languages)?;
        checkpoint(self.cancel)?;
        let origins = self.entry.with_texts(self.archive, &sources)?;
        let input = OriginalCatalogInput::new(self.forge, &origins);
        let texts = Texts::parse(&sources)?;
        let registry = CatalogTextRegistry::default();
        let projection = CatalogProjection::new(&texts, &input, &registry).for_kind(Kind::Ships);
        let files = ArchiveFiles::new(self.archive);
        let mut selected = project(groups, owners, &input, &projection, &files, self.cancel)?;
        let variant_sets = match scope {
            Scope::Sources => VariantSourceUseSets::default(),
            Scope::Canonical => super::variants::project(
                self.archive,
                &input,
                &mut selected.rows,
                &mut selected.coverage,
                self.cancel,
            )?,
        };
        let rows = projection.finish_rows()?;
        let source_inventory = origins.uses.finish_with(|| checkpoint(self.cancel))?;
        checkpoint(self.cancel)?;
        Ok(inspect(
            ShipSourceSnapshot {
                build: self.archive.build().cloned(),
                availability: if selected.rows.is_empty() {
                    self.declared_availability()
                } else {
                    DatasetAvailability::Available
                },
                rows: selected.rows,
                public_names: rows.public_names,
                game_texts: rows.game_texts,
                requested_languages: self.languages.to_vec(),
                available_languages: languages
                    .iter()
                    .copied()
                    .filter(|language| sources.entry(*language).is_some())
                    .collect(),
                localization_notes: texts.notes().to_vec(),
                source_inventory,
                coverage: selected.coverage,
                dependency_coverage: selected.dependencies,
            },
            variant_sets,
        ))
    }

    fn empty<T>(
        &self,
        coverage: CatalogRecordCoverage,
        inspect: impl FnOnce(ShipSourceSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        let mut dependencies = ShipSourceDependencyCoverage::default();
        dependencies.finish(self.archive, &ExportLedger::default())?;
        let source_inventory = self
            .entry
            .registry()
            .finish_with(|| checkpoint(self.cancel))?;
        checkpoint(self.cancel)?;
        Ok(inspect(ShipSourceSnapshot {
            build: self.archive.build().cloned(),
            availability: self.declared_availability(),
            rows: Vec::new(),
            public_names: Vec::new(),
            game_texts: Vec::new(),
            requested_languages: self.languages.to_vec(),
            available_languages: Vec::new(),
            localization_notes: Vec::new(),
            source_inventory,
            coverage,
            dependency_coverage: dependencies,
        }))
    }

    fn declared_availability(&self) -> DatasetAvailability {
        DatasetAvailability::from_source_declarations(self.forge, |name| {
            name == p4kx_assembly::ENTITY_TYPE
        })
    }

    fn owners(&self) -> Result<OwnerGroups<'query>, CatalogError> {
        if matches!(self.selection, CatalogRecordSelection::All) {
            return OwnerGroups::all(self.forge, self.cancel);
        }
        let plan = Plan::read(self.forge, Kind::Ships, self.selection, self.cancel, None)?;
        Ok(OwnerGroups::prepared(plan))
    }
}

fn project<'source>(
    mut groups: OwnerGroups<'source>,
    mut owners: Vec<Owner<'source>>,
    input: &'source OriginalCatalogInput<'source, 'source>,
    projection: &CatalogProjection<'_, 'source>,
    files: &'source ArchiveFiles<'source>,
    cancel: Option<&CancelToken>,
) -> Result<SelectedShips<'source>, CatalogError> {
    checkpoint(cancel)?;
    let entities = Arc::new(Entities::new(input.forge()));
    checkpoint(cancel)?;
    let pool = ShipPool::new(entities, files);
    let mut playability = ShipDependencies::new(input);
    let mut ledger = ExportLedger::default();
    let mut rows = Vec::new();
    let mut dependencies = ShipSourceDependencyCoverage::default();
    while !owners.is_empty() {
        let mut owner_ledger =
            playability.prepare(owners.iter().map(|owner| &owner.tree), cancel)?;
        let projector = ShipQueryProjector {
            input,
            projection,
            pool: &pool,
            playability: playability.evidence(),
            cancel,
        };
        for owner in owners.drain(..) {
            if let Some(row) = projector.project_owner(
                &owner,
                &mut owner_ledger,
                &mut groups.coverage,
                &mut dependencies,
            )? {
                rows.push(row);
            }
            ledger.merge(std::mem::take(&mut owner_ledger));
        }
        groups.fill(&mut owners, cancel)?;
    }
    dependencies.finish(files.archive(), &ledger)?;
    Ok(SelectedShips {
        rows,
        coverage: groups.finish(),
        dependencies,
    })
}

struct ShipQueryProjector<'context, 'source> {
    input: &'source OriginalCatalogInput<'source, 'source>,
    projection: &'context CatalogProjection<'context, 'source>,
    pool: &'context ShipPool<'source>,
    playability: &'context Evidence,
    cancel: Option<&'context CancelToken>,
}

impl<'source> ShipQueryProjector<'_, 'source> {
    fn project_owner(
        &self,
        owner: &Owner<'source>,
        ledger: &mut ExportLedger,
        coverage: &mut CatalogRecordCoverage,
        dependencies: &mut ShipSourceDependencyCoverage,
    ) -> Result<Option<ShipSource<'source>>, CatalogError> {
        checkpoint(self.cancel)?;
        let admission = RecordObservation::capture(self.input, &owner.record)?;
        ledger.merge(crate::export_field_walk::inspect_observed(
            Some(self.input),
            &owner.tree,
            &admission,
            Some(Kind::Ships),
            ExportSourceRole::Selected,
            self.cancel,
        )?);
        let projected = source_projection::read(
            self.pool,
            self.projection,
            self.input,
            &admission,
            &owner.tree,
            ledger,
            self.cancel,
        )?;
        if let Err(failure) = projected.read {
            if matches!(
                failure.canonical_error(),
                Some(DataForgeError::ObservationStopped)
            ) {
                return Err(CatalogError::Cancelled);
            }
            dependencies.record_failure(owner.record.index());
            coverage.incomplete_fields.push(owner.record.index());
            return Ok(None);
        }
        let aggregates = projected
            .aggregates
            .ok_or(ExportAccountingError::UnobservedFailure)?;
        let playability = projected.source.playability_proof.decide(
            &owner.tree,
            projected.source.vehicle,
            self.playability,
            self.input,
        )?;
        let row = ShipSource::from_read(projected.source, aggregates, playability)?;
        if !complete::complete(&row) {
            coverage.incomplete_fields.push(owner.record.index());
            dependencies.source_incomplete(owner.record.index());
        }
        Ok(Some(row))
    }
}
