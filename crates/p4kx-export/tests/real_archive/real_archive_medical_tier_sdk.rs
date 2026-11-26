// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::medical::MedicalItemTierSnapshot;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    ParsedCatalogQueries, ProvenanceMode,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, InputScope, Record, SourceLocation,
    TreeBuilder,
};
use serde_json::{Value, json};

use crate::real_archive_auto_spawns::provenance::Origins;
use crate::real_archive_catalog_table_parity::{TestResult, first_difference, wire_row};
use crate::real_archive_verification::print_line;

#[path = "real_archive_medical_tier_sdk_fields.rs"]
mod fields;

const FAMILY: &str = "MedicalItemTierConfig";
const BUILD: &str = "P4KX_MEDICAL_TIER_BUILD";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_medical_tier_sdk_matches_original_physical_definitions() -> TestResult {
    let path = p4kx_testkit::real_archive_path().map_err(|_| "configure the original Archive")?;
    let key = p4kx_testkit::real_archive_key().map_err(|_| "configure the Archive key")?;
    let archive =
        Archive::open_with_key(path, Some(key)).map_err(|_| "open the original Archive")?;
    let build = std::env::var(BUILD)?;
    if archive
        .build()
        .is_none_or(|actual| actual.change_number() != build)
    {
        return Err("the Archive differs from the nominated medical tier Build".into());
    }
    let watch = ArchiveWatch::new(&archive);
    watch.check()?;
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read the original DataForge Entry")?;
    let forge = entry.open().map_err(|_| "open the original DataForge")?;
    let original = original_records(&forge, &watch)?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?
    .with_provenance_mode(ProvenanceMode::Full);
    let parsed = session.parsed(None)?;
    if forge.record(u32::MAX).is_some() {
        return Err("expected a physical index outside the original Record table".into());
    }
    verify_missing(&archive, &parsed)?;
    let counts = parsed.with_medical_item_tier_configs(
        CatalogRecordSelection::All,
        None,
        |snapshot| {
            verify_coverage(&archive, &snapshot)?;
            let document = json!({"provenance": snapshot.source_inventory});
            let origins = Origins::read(&document, entry.origin())?;
            let counts = verify_original(&forge, &watch, &original, &snapshot, &origins)?;
            if let Some(row) = snapshot.rows.first() {
                let index = *row
                    .source()
                    .record_index()
                    .value()
                    .ok_or("selected physical tier index")?;
                verify_point(&archive, &parsed, index, &wire_row(row)?)?;
            }
            Ok::<_, Box<dyn std::error::Error>>(counts)
        },
    )??;
    watch.check()?;
    print_line(&format!(
        "medical tier SDK original parity: rows={} checkedCells={} tierSlots={}",
        counts.rows, counts.checked_cells, counts.tier_slots,
    ));
    Ok(())
}

fn original_records<'forge>(
    forge: &'forge DataForge<'_>,
    watch: &ArchiveWatch,
) -> TestResult<Vec<Record<'forge>>> {
    let mut owners = Vec::new();
    for record in forge.records() {
        watch.poll()?;
        if record
            .type_name()
            .map_err(|_| "classify the original medical tier family")?
            == FAMILY
        {
            owners.push(record);
        }
    }
    if owners.is_empty() {
        return Err("the nominated Build contains no physical medical tier definition".into());
    }
    owners.sort_by_cached_key(|record| (record.guid().to_string(), record.index()));
    Ok(owners)
}

#[derive(Default)]
struct Counts {
    rows: usize,
    checked_cells: usize,
    tier_slots: usize,
}

fn verify_original<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
    original: &[Record<'source>],
    sdk: &MedicalItemTierSnapshot<'_>,
    origins: &Origins<'_>,
) -> TestResult<Counts> {
    if original.len() != sdk.rows.len() {
        return Err("SDK and original medical tier family cardinality differ".into());
    }
    let mut counts = Counts::default();
    let mut trees = TreeBuilder::new();
    for (owner, projected) in original.iter().zip(&sdk.rows) {
        watch.poll()?;
        let header = projected.source();
        if header.guid().value() != Some(&owner.guid())
            || header.record_index().value() != Some(&owner.index())
            || header.source_type().value() != Some(&FAMILY)
            || header.record_path().value()
                != Some(
                    &owner
                        .record_path()
                        .map_err(|_| "read the original Record path")?,
                )
        {
            return Err("SDK medical tier physical Record or canonical order differs".into());
        }
        let record = trees
            .build(owner)
            .map_err(|_| "read the original medical tier Record")?;
        let wire = wire_row(projected)?;
        let _contributors = origins.with_resolved_contributors(&wire)?;
        let result = fields::verify(forge, &record, &wire, origins)?;
        counts.rows += 1;
        counts.checked_cells += result.checked_cells;
        counts.tier_slots += result.tier_slots;
    }
    if counts.tier_slots == 0 || counts.checked_cells == 0 {
        return Err(
            "the native fixture must exercise authored tier slots and original contributors".into(),
        );
    }
    Ok(counts)
}

fn verify_point(
    archive: &Archive,
    parsed: &ParsedCatalogQueries<'_>,
    index: u32,
    expected: &Value,
) -> TestResult {
    parsed.with_medical_item_tier_configs(
        CatalogRecordSelection::RecordIndex(index),
        None,
        |point| {
            verify_coverage(archive, &point)?;
            let [row] = point.rows.as_slice() else {
                return Err("a selected physical medical tier should return one row".into());
            };
            if let Some(path) = first_difference(expected, &wire_row(row)?) {
                return Err(
                    format!("medical tier full-family and exact query differ at {path}").into(),
                );
            }
            Ok(())
        },
    )?
}

fn verify_missing(archive: &Archive, parsed: &ParsedCatalogQueries<'_>) -> TestResult {
    parsed.with_medical_item_tier_configs(
        CatalogRecordSelection::RecordIndex(u32::MAX),
        None,
        |missing| {
            verify_coverage(archive, &missing)?;
            let mut uses = missing.source_inventory.iter_uses().peekable();
            if !missing.rows.is_empty()
                || uses.peek().is_none()
                || !uses.all(|usage| {
                    usage.scope() == InputScope::ArchiveDirectoryMetadata
                        && matches!(usage.location(), SourceLocation::Metadata { .. })
                })
            {
                return Err(
                    "the missing index should retain only Archive directory metadata".into(),
                );
            }
            Ok(())
        },
    )?
}

fn verify_coverage(archive: &Archive, sdk: &MedicalItemTierSnapshot<'_>) -> TestResult {
    let coverage = &sdk.coverage;
    if sdk.build.as_ref() != archive.build()
        || !matches!(sdk.availability, DatasetAvailability::Available)
        || !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
        || !coverage.incomplete_fields.is_empty()
        || coverage.skipped_leftovers != 0
    {
        return Err(format!(
            "medical tier SDK did not establish same-Build and complete coverage: headers={} bodies={} fields={} leftovers={}",
            coverage.unreadable_headers.len(), coverage.unreadable_bodies.len(),
            coverage.incomplete_fields.len(), coverage.skipped_leftovers,
        ).into());
    }
    Ok(())
}
