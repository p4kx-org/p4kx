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

pub(super) struct Query<'query> {
    pub(super) archive: &'query Archive,
    pub(super) entry: &'query DataForgeEntry,
    pub(super) forge: &'query DataForge<'query>,
    pub(super) selection: CatalogRecordSelection,
    pub(super) cancel: Option<&'query CancelToken>,
}

impl Query<'_> {
    pub(super) fn run(self) -> Result<LoadoutSnapshot, CatalogError> {
        let scope::Scope {
            wanted,
            mut coverage,
        } = scope::read(self.forge, self.selection, self.cancel)?;
        let origins = CatalogOrigins::for_archive_records(self.archive, self.entry)?;
        let (rows, dependencies) = self.resolve(&origins, wanted, &mut coverage)?;
        coverage.incomplete_fields.sort_unstable();
        coverage.incomplete_fields.dedup();
        let source_inventory = origins.uses.finish_with(|| checkpoint(self.cancel))?;
        checkpoint(self.cancel)?;
        let availability = if rows.is_empty() {
            DatasetAvailability::from_source_declarations(self.forge, |name| {
                name == p4kx_assembly::ENTITY_TYPE
            })
        } else {
            DatasetAvailability::Available
        };
        Ok(LoadoutSnapshot {
            build: self.archive.build().cloned(),
            availability,
            rows,
            source_inventory,
            coverage,
            dependencies,
        })
    }

    fn resolve(
        &self,
        origins: &CatalogOrigins,
        wanted: Vec<Wanted>,
        coverage: &mut CatalogRecordCoverage,
    ) -> Result<(Vec<Loadout>, LoadoutDependencyCoverage), CatalogError> {
        let mut resolver = EquipmentDependencyCoverage::default();
        let seed = ExportLedger::default();
        if wanted.is_empty() {
            resolver.finish(self.archive, &seed)?;
            return Ok((
                Vec::new(),
                LoadoutDependencyCoverage {
                    resolver,
                    alignment: Vec::new(),
                },
            ));
        }
        checkpoint(self.cancel)?;
        let input = OriginalCatalogInput::new(self.forge, origins);
        let locale = TextSources::read_languages(self.archive, &[])?;
        let texts = Texts::parse(&locale)?;
        let registry = CatalogTextRegistry::default();
        let projection = CatalogProjection::new(&texts, &input, &registry).for_kind(Kind::Loadouts);
        let mut alignment = DirectAlignment::new(&projection, self.cancel);
        let files = ArchiveFiles::new(self.archive);
        let entities = Arc::new(Entities::new(self.forge));
        let mut rows = Vec::new();
        let read = resolution::read_rows(
            &files,
            &entities,
            wanted.into_iter(),
            ResolverEvidence {
                seed: &seed,
                alignment: &mut alignment,
            },
            &input,
            self.cancel,
            |row| {
                if !row.port_inventory_complete || !row.issues.is_empty() {
                    coverage.incomplete_fields.push(row.root_record_index);
                }
                rows.push(row);
                Ok(())
            },
        )?;
        for failure in read.failures {
            let index = failure.record().index();
            coverage.incomplete_fields.push(index);
            resolver.record_failure(index);
        }
        resolver.finish(self.archive, &read.ledger)?;
        Ok((
            rows,
            LoadoutDependencyCoverage {
                resolver,
                alignment: alignment.dependencies().collect(),
            },
        ))
    }
}
