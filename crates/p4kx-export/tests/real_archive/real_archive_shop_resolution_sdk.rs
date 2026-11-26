// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::shops::{
    self, ResolutionStatus, ShopInventoryResolution, ShopInventoryResolutionSnapshot,
    ShopResolutionSelection,
};
use p4kx_export::catalog_queries::{DatasetAvailability, ProvenanceMode};
use p4kx_extraction::{Archive, ArchiveWatch, PhysicalSourceKind, SourceLocation, SourceUseRole};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_catalog_table_parity::{
    self as parity, BoundMeasurement, Origins, TestResult,
};
use crate::real_archive_verification::print_line;

const TABLE: &str = "/declarations/shopInventoryResolution";
const COLD_KEY: &str = "P4KX_SHOP_RESOLUTION_KEY";

#[test]
#[ignore = "same Build: set P4KX_DATA_P4K and P4KX_CATALOG_MEASUREMENT_DIR"]
fn installed_shop_resolution_matches_the_same_complete_catalog_and_exact_key() -> TestResult {
    parity::with_bound_measurement(verify)
}

fn verify(archive: &Archive, measurement: &BoundMeasurement) -> TestResult {
    let all = read(archive, ShopResolutionSelection::All)?;
    let sample = Sample::capture(&all)?;
    let mut origins = Origins::default();
    let table = parity::capture_rows(
        all.availability,
        &all.rows,
        &all.source_inventory,
        &mut origins,
        shop_key,
    )?;
    report("all", &all)?;
    drop(all);

    let compared = measurement.verify_tables([TABLE], &[table], &origins)?;
    let [rows] = compared.rows;
    let exact = read(archive, ShopResolutionSelection::Key(&sample.key))?;
    sample.verify(&exact)?;
    report("key", &exact)?;
    print_line(&format!(
        "complete Shop resolution SDK parity: rows={rows} sourceUses={} physicalSources={}",
        compared.source_uses, compared.physical_sources,
    ));
    Ok(())
}

#[test]
#[ignore = "real Archive: measure a fresh process per Build; P4KX_SHOP_RESOLUTION_KEY selects one key"]
fn installed_shop_resolution_cold_query() -> TestResult {
    let key = match std::env::var(COLD_KEY) {
        Ok(key) => Some(key),
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err("invalid private Shop selector".into());
        }
    };
    let path = real_archive_path().map_err(|_| "configure the installed Archive")?;
    let archive_key = real_archive_key().map_err(|_| "configure the Archive key")?;
    let archive = Archive::open_with_key(path, Some(archive_key))
        .map_err(|_| "open the nominated Archive")?;
    let watch = ArchiveWatch::new(&archive);
    watch
        .check()
        .map_err(|_| "Archive changed before the cold query")?;
    let selection = key
        .as_deref()
        .map_or(ShopResolutionSelection::All, ShopResolutionSelection::Key);
    let snapshot = read(&archive, selection)?;
    watch
        .check()
        .map_err(|_| "Archive changed during the cold query")?;
    match key.as_ref() {
        Some(wanted) => {
            require_key(&snapshot, wanted)?;
            report("cold-key", &snapshot)
        }
        None => report("cold-all", &snapshot),
    }
}

fn read(
    archive: &Archive,
    selection: ShopResolutionSelection<'_>,
) -> TestResult<ShopInventoryResolutionSnapshot> {
    let snapshot = shops::read_shop_inventory_resolutions_with_provenance_mode(
        archive,
        selection,
        ProvenanceMode::Full,
        None,
    )
    .map_err(|_| "read the requested Shop resolution SDK scope")?;
    if archive.build().is_none()
        || snapshot.build.as_ref() != archive.build()
        || !matches!(snapshot.availability, DatasetAvailability::Available)
        || snapshot.rows.is_empty()
    {
        return Err(
            "the SDK scope must return actual registry rows from the nominated Build".into(),
        );
    }
    check_root_directory(archive, &snapshot)?;
    Ok(snapshot)
}

fn check_root_directory(
    archive: &Archive,
    snapshot: &ShopInventoryResolutionSnapshot,
) -> TestResult {
    let directory = &snapshot
        .coverage
        .discovery
        .as_ref()
        .ok_or("selected registry rows require original declaration discovery")?
        .inventory_directory;
    let [id] = directory.source_uses_refs() else {
        return Err("exactly one source use must prove the Archive directory survey".into());
    };
    let usage = snapshot
        .source_inventory
        .iter_uses()
        .find(|usage| usage.id() == id)
        .ok_or("unresolved Shop directory source use")?;
    let source = snapshot
        .source_inventory
        .iter_sources()
        .find(|source| source.id() == usage.source())
        .ok_or("unresolved physical Shop directory source")?;
    let PhysicalSourceKind::ArchiveDirectory(source) = source.kind() else {
        return Err("the survey must bind to the original root Archive directory".into());
    };
    let SourceLocation::ArchiveDirectoryPaths {
        start_entry_index,
        end_entry_index,
    } = usage.location()
    else {
        return Err("the selector must retain its original directory path range".into());
    };
    let expected = u64::try_from(archive.entries().len())?;
    if !directory.source_set_complete()
        || usage.role() != SourceUseRole::Selector
        || start_entry_index.get() != 0
        || end_entry_index.get() != expected
        || source.admitted_entry_count().get() != expected
    {
        return Err("the source does not prove a complete original root directory survey".into());
    }
    Ok(())
}

struct Sample {
    key: String,
    value: Value,
    origins: Origins,
}

impl Sample {
    fn capture(snapshot: &ShopInventoryResolutionSnapshot) -> TestResult<Self> {
        let row = snapshot
            .rows
            .iter()
            .find(|row| {
                row.status().value() == Some(&ResolutionStatus::Resolved)
                    && row
                        .physical_targets()
                        .value()
                        .is_some_and(|targets| matches!(targets.as_slice(), [_]))
            })
            .ok_or("a resolved Shop row with one physical target is required for Key replay")?;
        let key = row
            .shop_key()
            .value()
            .ok_or("original admitted Shop key")?
            .clone();
        let (value, origins) = row_with_origins(row, snapshot)?;
        Ok(Self {
            key,
            value,
            origins,
        })
    }

    fn verify(&self, snapshot: &ShopInventoryResolutionSnapshot) -> TestResult {
        let row = require_key(snapshot, &self.key)?;
        let (value, origins) = row_with_origins(row, snapshot)?;
        if value != self.value || origins != self.origins {
            return Err(
                "the Key query differs from the verified All row and source closure".into(),
            );
        }
        Ok(())
    }
}

fn require_key<'row>(
    snapshot: &'row ShopInventoryResolutionSnapshot,
    expected: &str,
) -> TestResult<&'row ShopInventoryResolution> {
    match snapshot.rows.as_slice() {
        [row] if row.shop_key().value().is_some_and(|key| key == expected) => Ok(row),
        _ => Err("the SDK must reproduce exactly the nominated Shop key".into()),
    }
}

fn row_with_origins(
    row: &ShopInventoryResolution,
    snapshot: &ShopInventoryResolutionSnapshot,
) -> TestResult<(Value, Origins)> {
    let value = parity::wire_row(row)?;
    let mut origins = Origins::default();
    origins.extend(&snapshot.source_inventory, std::slice::from_ref(&value))?;
    Ok((value, origins))
}

fn shop_key(row: &Value) -> TestResult<String> {
    row.pointer("/shopKey/value")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "resolved original Shop row identity is required".into())
}

fn report(phase: &str, snapshot: &ShopInventoryResolutionSnapshot) -> TestResult {
    let discovery = snapshot
        .coverage
        .discovery
        .as_ref()
        .ok_or("original discovery observations")?;
    print_line(&format!(
        "Shop resolution SDK {phase}: rows={} uniqueBodies={} registryComplete={} declarationsComplete={} failedDependencies={}",
        snapshot.rows.len(),
        discovery.inventory_payloads,
        snapshot.coverage.registry_complete,
        discovery.declarations_complete,
        snapshot.coverage.entries.failed_dependencies,
    ));
    Ok(())
}
