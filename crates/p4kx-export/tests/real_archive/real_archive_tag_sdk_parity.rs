// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;

use p4kx_export::catalog_queries::tags::{TagDeclarationFamily, TagDeclarationRows};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection, DatasetAvailability,
    ParsedCatalogQueries,
};
use p4kx_extraction::{Archive, Build, SourceInventory};
use serde::Serialize;

use crate::real_archive_catalog_table_parity::{
    self, DefinitionOwner, StreamedDefinitionDigests, TestResult,
};
use crate::real_archive_scoped_equipment_sdk::{component_family_parity, equipment_input};
use crate::real_archive_verification;

#[derive(Clone, Copy)]
enum Scope {
    Definitions,
    Roots,
    Paths,
}

const SCOPES: [Scope; 3] = [Scope::Definitions, Scope::Roots, Scope::Paths];
const DATASET: &str = "P4KX_TAG_SDK_COLD_DATASET";
const PRODUCER: &str = "P4KX_TAG_CATALOG_PRODUCER_REVISION";

#[test]
#[ignore = "installed Archive: configure the exact Build and one canonical tag dataset"]
fn installed_tag_cold_query() -> TestResult {
    let path = env::var(DATASET)?;
    let scope = SCOPES
        .into_iter()
        .find(|scope| scope.path() == path)
        .ok_or("expected one canonical tag dataset path")?;
    equipment_input::with_archive(|archive| {
        let session =
            equipment_input::full_source_session(archive, CatalogLanguageSelection::Selected(&[]))?;
        scope.read(archive, &session.parsed(None)?, None)
    })
}

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_tags_match_one_complete_catalog() -> TestResult {
    let producer = env::var(PRODUCER)?;
    real_archive_catalog_table_parity::with_bound_measurement_from_producer(
        &producer,
        |archive, measurement| {
            let scratch = tempfile::tempdir()?;
            let mut evidence = StreamedDefinitionDigests::new(
                scratch.path(),
                [DefinitionOwner::Record; SCOPES.len()],
            );
            {
                let session = equipment_input::full_source_session(
                    archive,
                    CatalogLanguageSelection::Selected(&[]),
                )?;
                let parsed = session.parsed(None)?;
                for scope in SCOPES {
                    scope.read(archive, &parsed, Some(&mut evidence))?;
                }
            }
            let counts = evidence.compare(measurement, SCOPES.map(Scope::path))?;
            real_archive_verification::print_line(&format!(
                "complete Tag SDK parity: rows={:?} sourceUses={} physicalSources={}",
                counts.rows, counts.source_uses, counts.physical_sources,
            ));
            Ok(())
        },
    )
}

impl Scope {
    const fn path(self) -> &'static str {
        match self {
            Self::Definitions => "/definitions/tags",
            Self::Roots => "/declarations/tagRoots",
            Self::Paths => "/projections/tagPaths",
        }
    }

    const fn position(self) -> usize {
        match self {
            Self::Definitions => 0,
            Self::Roots => 1,
            Self::Paths => 2,
        }
    }

    fn read(
        self,
        archive: &Archive,
        parsed: &ParsedCatalogQueries<'_>,
        evidence: Option<&mut StreamedDefinitionDigests<'_, { SCOPES.len() }>>,
    ) -> TestResult {
        let all = CatalogRecordSelection::All;
        let family = match self {
            Self::Definitions => TagDeclarationFamily::Definitions,
            Self::Roots => TagDeclarationFamily::Roots,
            Self::Paths => {
                return parsed.with_tag_paths(all, None, |snapshot| {
                    require_build(archive, snapshot.build.as_ref())?;
                    if !snapshot.graph_inventory_complete() {
                        return Err("incomplete tag graph dependency coverage".into());
                    }
                    self.observe(
                        snapshot.availability,
                        &snapshot.rows,
                        &snapshot.source_inventory,
                        evidence,
                    )
                })?;
            }
        };
        parsed.with_tag_declarations(family, all, None, |snapshot| {
            require_build(archive, snapshot.build.as_ref())?;
            let coverage = &snapshot.coverage;
            if coverage.skipped_leftovers != 0 {
                return Err("unexpected classification exclusion in tag declaration scope".into());
            }
            let inventory = &snapshot.source_inventory;
            match (self, snapshot.definitions, snapshot.roots) {
                (Self::Definitions, Some(table), None) => {
                    self.observe_declaration(&table, inventory, coverage, evidence)
                }
                (Self::Roots, None, Some(table)) => {
                    self.observe_declaration(&table, inventory, coverage, evidence)
                }
                _ => Err("expected exactly one requested tag declaration family".into()),
            }
        })?
    }

    fn observe_declaration<Row: Serialize>(
        self,
        table: &TagDeclarationRows<Row>,
        inventory: &SourceInventory,
        coverage: &CatalogRecordCoverage,
        evidence: Option<&mut StreamedDefinitionDigests<'_, { SCOPES.len() }>>,
    ) -> TestResult {
        component_family_parity::require_complete(
            self.path(),
            table.availability,
            table.rows.len(),
            coverage,
        )?;
        self.observe(table.availability, &table.rows, inventory, evidence)
    }

    fn observe<Row: Serialize>(
        self,
        availability: DatasetAvailability,
        rows: &[Row],
        inventory: &SourceInventory,
        evidence: Option<&mut StreamedDefinitionDigests<'_, { SCOPES.len() }>>,
    ) -> TestResult {
        let uses = inventory.use_count();
        let sources = inventory.source_count();
        let row_evidence = match (availability, rows.is_empty()) {
            (DatasetAvailability::Available, false) if uses != 0 && sources != 0 => "present",
            (DatasetAvailability::Available, true) => "complete-empty",
            (DatasetAvailability::Absent, true) => "absent-type",
            _ => {
                return Err(
                    "tag row availability or original source inventory is incomplete".into(),
                );
            }
        };
        if let Some(evidence) = evidence {
            evidence.retain(self.position(), availability, rows, inventory)?;
        }
        let path = self.path();
        real_archive_verification::print_line(&format!(
            "Tag SDK: dataset={path} availability={availability:?} rowEvidence={row_evidence} rows={} inventoryUses={uses} inventorySources={sources} skippedLeftovers=0",
            rows.len(),
        ));
        Ok(())
    }
}

fn require_build(archive: &Archive, build: Option<&Build>) -> TestResult {
    if build.is_none() || build != archive.build() {
        return Err("the tag SDK snapshot must match the installed Build".into());
    }
    Ok(())
}
