// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{ArchiveFiles, Entities};
use p4kx_extraction::{Archive, CancelToken, DataForge, DataForgeEntry};

use super::super::dependency_coverage::EquipmentDependencyCoverage;
use super::{LoadoutDependencyCoverage, LoadoutSnapshot, scope};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection, DatasetAvailability};
use crate::catalog_query_session::checkpoint;
use crate::contract_loadout::Loadout;
use crate::export_ledger::ExportLedger;
use crate::loadout::Wanted;
use crate::loadout::alignment::direct::DirectAlignment;
use crate::loadout::resolution::{self, ResolverEvidence};
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, Kind};

pub(in crate::build::catalog_queries::equipment) struct Query<'query> {
    pub(in crate::build::catalog_queries::equipment) archive: &'query Archive,
    pub(in crate::build::catalog_queries::equipment) entry: &'query DataForgeEntry,
    pub(in crate::build::catalog_queries::equipment) forge: &'query DataForge<'query>,
    pub(in crate::build::catalog_queries::equipment) selection: CatalogRecordSelection,
    pub(in crate::build::catalog_queries::equipment) cancel: Option<&'query CancelToken>,
}

pub(super) struct ResolverInputs<'source> {
    files: ArchiveFiles<'source>,
    entities: Arc<Entities<'source>>,
}

impl<'source> ResolverInputs<'source> {
    pub(super) fn new(archive: &'source Archive, forge: &'source DataForge<'source>) -> Self {
        Self {
            files: ArchiveFiles::new(archive),
            entities: Arc::new(Entities::new(forge)),
        }
    }
}

impl Query<'_> {
    pub(super) fn run(self) -> Result<LoadoutSnapshot, CatalogError> {
        let scope::Scope {
            wanted,
            mut coverage,
        } = scope::read(self.forge, self.selection, self.cancel)?;
        let origins = CatalogOrigins::for_archive_records(self.archive, self.entry)?;
        let (rows, dependencies) = self.resolve(&origins, wanted, &mut coverage)?;
        scope::finish_coverage(&mut coverage);
        let source_inventory = origins.uses.finish_with(|| checkpoint(self.cancel))?;
        checkpoint(self.cancel)?;
        let availability = self.availability(rows.len());
        Ok(LoadoutSnapshot {
            build: self.archive.build().cloned(),
            availability,
            rows,
            source_inventory,
            coverage,
            dependencies,
        })
    }

    pub(super) fn availability(&self, row_count: usize) -> DatasetAvailability {
        if row_count == 0 {
            DatasetAvailability::from_source_declarations(self.forge, |name| {
                name == p4kx_assembly::ENTITY_TYPE
            })
        } else {
            DatasetAvailability::Available
        }
    }

    pub(super) fn resolve(
        &self,
        origins: &CatalogOrigins,
        wanted: Vec<Wanted>,
        coverage: &mut CatalogRecordCoverage,
    ) -> Result<(Vec<Loadout>, LoadoutDependencyCoverage), CatalogError> {
        checkpoint(self.cancel)?;
        let input = OriginalCatalogInput::new(self.forge, origins);
        let locale = TextSources::read_languages(self.archive, &[])?;
        let texts = Texts::parse(&locale)?;
        let registry = CatalogTextRegistry::default();
        let projection = CatalogProjection::new(&texts, &input, &registry).for_kind(Kind::Loadouts);
        let mut alignment = DirectAlignment::new(&projection, self.cancel);
        self.resolve_rows(&input, wanted, coverage, &mut alignment)
    }

    pub(in crate::build::catalog_queries::equipment) fn resolve_rows(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
        wanted: Vec<Wanted>,
        coverage: &mut CatalogRecordCoverage,
        alignment: &mut DirectAlignment<'_>,
    ) -> Result<(Vec<Loadout>, LoadoutDependencyCoverage), CatalogError> {
        if wanted.is_empty() {
            let mut resolver = EquipmentDependencyCoverage::default();
            resolver.finish(self.archive, &ExportLedger::default())?;
            return Ok((
                Vec::new(),
                LoadoutDependencyCoverage {
                    resolver,
                    alignment: Vec::new(),
                },
            ));
        }
        let inputs = ResolverInputs::new(self.archive, self.forge);
        let mut rows = Vec::new();
        let dependencies = self.resolve_prepared_rows(
            input,
            &inputs,
            wanted.into_iter(),
            coverage,
            alignment,
            |row| {
                rows.push(row);
                Ok(())
            },
        )?;
        Ok((rows, dependencies))
    }

    pub(super) fn resolve_prepared_rows(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
        sources: &ResolverInputs<'_>,
        wanted: impl Iterator<Item = Wanted>,
        coverage: &mut CatalogRecordCoverage,
        alignment: &mut DirectAlignment<'_>,
        mut accept: impl FnMut(Loadout) -> Result<(), CatalogError>,
    ) -> Result<LoadoutDependencyCoverage, CatalogError> {
        let mut resolver = EquipmentDependencyCoverage::default();
        let seed = ExportLedger::default();
        let read = resolution::read_rows(
            &sources.files,
            &sources.entities,
            wanted,
            ResolverEvidence {
                seed: &seed,
                alignment,
            },
            input,
            self.cancel,
            |row| {
                if !row.port_inventory_complete || !row.issues.is_empty() {
                    coverage.incomplete_fields.push(row.root_record_index);
                }
                accept(row)
            },
        )?;
        for failure in read.failures {
            let index = failure.record().index();
            coverage.incomplete_fields.push(index);
            resolver.record_failure(index);
        }
        resolver.finish(self.archive, &read.ledger)?;
        Ok(LoadoutDependencyCoverage {
            resolver,
            alignment: alignment.dependencies().collect(),
        })
    }
}
