// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    DatasetAvailability, ParsedCatalogQueries, ResourceGroupParentSnapshot, ResourceGroupSnapshot,
};
use p4kx_extraction::{Archive, Build, SourceInventory};
use serde_json::Value;

use super::equipment_input;
use crate::real_archive_catalog_table_parity::{self, Origins, TestResult};
use crate::real_archive_verification;

const TABLES: [&str; 3] = [
    "/definitions/resourceGroups",
    "/declarations/resourceGroupMembers",
    "/projections/resourceGroupParents",
];

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_resource_group_tables_match_one_complete_catalog() -> TestResult {
    let revision = env::var("P4KX_RESOURCE_GROUP_CATALOG_PRODUCER_REVISION")
        .map_err(|_| "configure the exact ResourceGroup Catalog producer revision")?;
    real_archive_catalog_table_parity::with_bound_measurement_from_producer(
        &revision,
        |archive, measurement| {
            let session =
                CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)?;
            let parsed = session.parsed(None)?;
            let mut origins = Origins::default();
            let (definitions, members) = with_direct(archive, &parsed, |sdk| {
                let definitions = real_archive_catalog_table_parity::capture_rows(
                    sdk.availability(),
                    sdk.definitions(),
                    sdk.source_inventory(),
                    &mut origins,
                    physical_identity,
                )?;
                let members = real_archive_catalog_table_parity::capture_rows(
                    sdk.availability(),
                    sdk.members(),
                    sdk.source_inventory(),
                    &mut origins,
                    physical_identity,
                )?;
                Ok((definitions, members))
            })?;
            let parents = with_parents(archive, &parsed, |sdk| {
                require_same_owners(
                    definitions.rows().iter().map(physical_index),
                    sdk.rows()
                        .iter()
                        .map(|row| row.record_index().value().copied()),
                )?;
                real_archive_catalog_table_parity::capture_rows(
                    sdk.availability(),
                    sdk.rows(),
                    sdk.source_inventory(),
                    &mut origins,
                    physical_identity,
                )
            })?;
            let compared =
                measurement.verify_tables(TABLES, &[definitions, members, parents], &origins)?;
            real_archive_verification::print_line(&format!(
                "complete ResourceGroup SDK parity: rows={:?} sourceUses={} physicalSources={}",
                compared.rows, compared.source_uses, compared.physical_sources,
            ));
            Ok(())
        },
    )
}

enum ColdScope {
    Direct,
    Parents,
}

#[test]
#[ignore = "installed Archive: configure the expected Build and direct or parents scope"]
fn installed_resource_group_cold_query() -> TestResult {
    let scope = env::var("P4KX_RESOURCE_GROUP_SDK_COLD_SCOPE")
        .map_err(|_| "configure the direct or parents ResourceGroup scope")?;
    let scope = match scope.as_str() {
        "direct" => ColdScope::Direct,
        "parents" => ColdScope::Parents,
        _ => return Err("expected exactly the direct or parents ResourceGroup scope".into()),
    };
    equipment_input::with_archive(|archive| {
        let session =
            CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)?;
        let parsed = session.parsed(None)?;
        match scope {
            ColdScope::Direct => with_direct(archive, &parsed, |sdk| {
                cold_witness(
                    "direct",
                    sdk.availability(),
                    sdk.definitions().len(),
                    sdk.source_inventory(),
                )
            }),
            ColdScope::Parents => with_parents(archive, &parsed, |sdk| {
                cold_witness(
                    "parents",
                    sdk.availability(),
                    sdk.rows().len(),
                    sdk.source_inventory(),
                )
            }),
        }
    })
}

fn with_direct<T>(
    archive: &Archive,
    parsed: &ParsedCatalogQueries<'_>,
    inspect: impl FnOnce(&ResourceGroupSnapshot<'_>) -> TestResult<T>,
) -> TestResult<T> {
    parsed.with_resource_groups(CatalogRecordSelection::All, None, |sdk| {
        require_snapshot(
            archive.build(),
            sdk.build(),
            sdk.availability(),
            sdk.definitions().len(),
            sdk.coverage(),
            "ResourceGroup direct query",
        )?;
        require_same_owners(
            sdk.definitions()
                .iter()
                .map(|row| row.header().record_index().value().copied()),
            sdk.members()
                .iter()
                .map(|row| row.record_index().value().copied()),
        )?;
        real_archive_verification::print_line(&format!(
            "ResourceGroup SDK direct: availability={:?} definitions={} members={}",
            sdk.availability(),
            sdk.definitions().len(),
            sdk.members().len(),
        ));
        inspect(&sdk)
    })?
}

fn with_parents<T>(
    archive: &Archive,
    parsed: &ParsedCatalogQueries<'_>,
    inspect: impl FnOnce(&ResourceGroupParentSnapshot) -> TestResult<T>,
) -> TestResult<T> {
    parsed.with_resource_group_parents(CatalogRecordSelection::All, None, |sdk| {
        require_snapshot(
            archive.build(),
            sdk.build(),
            sdk.availability(),
            sdk.rows().len(),
            sdk.coverage(),
            "ResourceGroup parent query",
        )?;
        require_coverage(
            sdk.dependency_coverage(),
            "ResourceGroup parent dependency survey",
        )?;
        if sdk.rows().iter().any(|row| {
            row.parent().value().is_none() || !row.parent().origin().source_set_complete()
        }) {
            return Err(
                "a ResourceGroup parent choice lacks complete original selector evidence".into(),
            );
        }
        real_archive_verification::print_line(&format!(
            "ResourceGroup SDK parents: availability={:?} rows={}",
            sdk.availability(),
            sdk.rows().len(),
        ));
        inspect(&sdk)
    })?
}

fn require_snapshot(
    expected_build: Option<&Build>,
    observed_build: Option<&Build>,
    availability: DatasetAvailability,
    rows: usize,
    coverage: &CatalogRecordCoverage,
    scope: &str,
) -> TestResult {
    if observed_build.is_none() || observed_build != expected_build {
        return Err(format!("{scope} requires the exact installed Build").into());
    }
    require_availability(availability, rows, scope)?;
    require_coverage(coverage, scope)
}

fn require_availability(availability: DatasetAvailability, rows: usize, scope: &str) -> TestResult {
    match availability {
        DatasetAvailability::Available => Ok(()),
        DatasetAvailability::Absent if rows == 0 => Ok(()),
        _ => Err(format!("{scope} could not establish source-family availability").into()),
    }
}

fn require_coverage(coverage: &CatalogRecordCoverage, scope: &str) -> TestResult {
    if !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
        || !coverage.incomplete_fields.is_empty()
    {
        return Err(format!(
            "{scope} incomplete (unreadableHeaders={} unreadableBodies={} incompleteFields={})",
            coverage.unreadable_headers.len(),
            coverage.unreadable_bodies.len(),
            coverage.incomplete_fields.len(),
        )
        .into());
    }
    Ok(())
}

fn require_same_owners(
    expected: impl ExactSizeIterator<Item = Option<u32>>,
    observed: impl ExactSizeIterator<Item = Option<u32>>,
) -> TestResult {
    if expected.len() != observed.len() {
        return Err("ResourceGroup tables have different physical owner counts".into());
    }
    for (position, (expected, observed)) in expected.zip(observed).enumerate() {
        if expected.is_none() || expected != observed {
            return Err(format!(
                "ResourceGroup tables disagree on the physical owner at row {position}"
            )
            .into());
        }
    }
    Ok(())
}

fn physical_index(row: &Value) -> Option<u32> {
    row.pointer("/recordIndex/value")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

fn physical_identity(row: &Value) -> TestResult<u32> {
    physical_index(row)
        .ok_or_else(|| "expected an original ResourceGroup physical Record index".into())
}

fn cold_witness(
    scope: &str,
    availability: DatasetAvailability,
    rows: usize,
    inventory: &SourceInventory,
) -> TestResult {
    let uses = inventory.use_count();
    let sources = inventory.source_count();
    if rows > 0 && (uses == 0 || sources == 0) {
        return Err(format!("ResourceGroup {scope} rows have an empty source inventory").into());
    }
    real_archive_verification::print_line(&format!(
        "ResourceGroup SDK cold: scope={scope} availability={availability:?} rows={rows} inventoryUses={uses} inventorySources={sources}",
    ));
    Ok(())
}

#[test]
fn resource_group_parity_preserves_physical_aliases_and_rejects_missing_owners() -> TestResult {
    use serde_json::json;

    let aliases = [
        json!({"recordIndex": {"value": 3}, "guid": {"value": "same"}}),
        json!({"recordIndex": {"value": 4}, "guid": {"value": "same"}}),
    ];
    require_same_owners(
        aliases.iter().map(physical_index),
        [Some(3), Some(4)].into_iter(),
    )?;
    assert!(
        require_same_owners(
            aliases.iter().map(physical_index),
            [Some(4), Some(3)].into_iter()
        )
        .is_err(),
        "a reused GUID cannot hide physical owner order changes",
    );
    assert!(
        require_same_owners(aliases.iter().map(physical_index), [Some(3)].into_iter()).is_err(),
        "a missing alias cannot establish the joint owner sequence",
    );
    assert!(
        physical_identity(&json!({"parent": {"targetRecordIndex": {"value": 4}}})).is_err(),
        "a target header cannot identify the physical child owner",
    );
    Ok(())
}

#[test]
fn resource_group_parity_rejects_incomplete_dependencies_and_false_absence() -> TestResult {
    let mut coverage = CatalogRecordCoverage::default();
    require_coverage(&coverage, "parents")?;
    require_availability(DatasetAvailability::Absent, 0, "parents")?;
    require_availability(DatasetAvailability::Available, 0, "parents")?;
    assert!(
        require_availability(DatasetAvailability::Absent, 1, "parents").is_err(),
        "a projected child contradicts family absence",
    );
    assert!(
        require_availability(DatasetAvailability::Unavailable, 0, "parents").is_err(),
        "an unavailable family cannot establish an empty parent table",
    );
    coverage.incomplete_fields.push(4);
    assert!(
        require_coverage(&coverage, "parents").is_err(),
        "an incomplete dependency can change the selected parent",
    );
    Ok(())
}
