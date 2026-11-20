// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::law::{LawFamily, LawRows, LawSnapshot};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use p4kx_extraction::{Archive, ArchiveWatch, DataForgeEntry};
use serde::Serialize;
use serde_json::Value;

use crate::real_archive_catalog_table_parity::{
    Origins, Table, TestResult, capture_rows, verify_tables,
};
use crate::real_archive_verification::print_line;

#[path = "real_archive_law_sdk_original.rs"]
mod original;
use original::{FAMILIES, OriginalLaws};

const BUILD: &str = "P4KX_LAW_SDK_BUILD";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_law_sdk_matches_original_physical_definitions() -> TestResult {
    let path = p4kx_testkit::real_archive_path().map_err(|_| "configure the original Archive")?;
    let key = p4kx_testkit::real_archive_key().map_err(|_| "configure the original Archive key")?;
    let archive =
        Archive::open_with_key(path, Some(key)).map_err(|_| "open the original Archive")?;
    inspect_laws(&archive, |_, counts| {
        print_line(&format!(
            "installed law SDK source coverage: family rows={counts:?}"
        ));
        Ok(())
    })
}

#[test]
#[ignore = "completed normal installed Catalog: set P4KX_DATA_P4K and P4KX_CATALOG_MEASUREMENT_DIR"]
fn installed_law_sdk_matches_the_same_complete_catalog() -> TestResult {
    let parity = verify_tables(FAMILIES.map(|family| family.path), |archive, origins| {
        inspect_laws(archive, |sdk, _| capture_tables(sdk, origins))
    })?;
    print_line(&format!(
        "complete law SDK parity: family rows={:?} sourceUses={} physicalSources={}",
        parity.rows, parity.source_uses, parity.physical_sources,
    ));
    Ok(())
}

fn inspect_laws<T>(
    archive: &Archive,
    inspect: impl FnOnce(&LawSnapshot<'_>, [usize; 4]) -> TestResult<T>,
) -> TestResult<T> {
    let expected = std::env::var(BUILD)?;
    if archive
        .build()
        .is_none_or(|build| build.change_number() != expected)
    {
        return Err("Archive does not match the selected law SDK Build receipt".into());
    }
    let watch = ArchiveWatch::new(archive);
    watch.check()?;
    let entry =
        DataForgeEntry::read(archive).map_err(|_| "read the original law DataForge Entry")?;
    let forge = entry
        .open()
        .map_err(|_| "open the original law DataForge")?;
    let original = OriginalLaws::read(&forge, &watch)?;
    let session = CatalogQuerySession::from_entry(
        archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let parsed = session.parsed(None)?;
    verify_known_absence(archive, &parsed, &original)?;
    let result = parsed.with_law(LawFamily::All, CatalogRecordSelection::All, None, |sdk| {
        verify_health(archive, &sdk)?;
        let counts = original.verify(&sdk, &forge, entry.origin(), &watch)?;
        inspect(&sdk, counts)
    })?;
    watch.check()?;
    result
}

fn verify_known_absence(
    archive: &Archive,
    parsed: &ParsedCatalogQueries<'_>,
    original: &OriginalLaws<'_>,
) -> TestResult {
    parsed.with_law(
        LawFamily::All,
        CatalogRecordSelection::RecordIndex(u32::MAX),
        None,
        |sdk| {
            verify_health(archive, &sdk)?;
            if !sdk.available_languages.is_empty()
                || !sdk.public_names.is_empty()
                || !sdk.game_texts.is_empty()
            {
                return Err("proven empty Record selection opened unrelated locale rows".into());
            }
            original.verify_absence(&sdk)
        },
    )?
}

fn verify_health(archive: &Archive, sdk: &LawSnapshot<'_>) -> TestResult {
    if sdk.build.as_ref() != archive.build() {
        return Err("law SDK snapshot came from a different Build".into());
    }
    let coverage = &sdk.coverage;
    if !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
        || !coverage.incomplete_fields.is_empty()
        || coverage.skipped_leftovers != 0
    {
        return Err(format!(
            "law SDK coverage diagnostics: headers={} bodies={} incompleteFields={} leftovers={}",
            coverage.unreadable_headers.len(),
            coverage.unreadable_bodies.len(),
            coverage.incomplete_fields.len(),
            coverage.skipped_leftovers,
        )
        .into());
    }
    Ok(())
}

fn capture_tables(sdk: &LawSnapshot<'_>, origins: &mut Origins) -> TestResult<[Table; 4]> {
    Ok([
        capture(sdk.infractions.as_ref(), sdk, origins)?,
        capture(sdk.infraction_sets.as_ref(), sdk, origins)?,
        capture(sdk.jurisdictions.as_ref(), sdk, origins)?,
        capture(sdk.security_network_manifests.as_ref(), sdk, origins)?,
    ])
}

fn capture<Row: Serialize>(
    table: Option<&LawRows<Row>>,
    sdk: &LawSnapshot<'_>,
    origins: &mut Origins,
) -> TestResult<Table> {
    let table = table.ok_or("requested canonical law table")?;
    capture_rows(
        table.availability,
        &table.rows,
        &sdk.source_inventory,
        origins,
        |row| {
            row.pointer("/recordIndex/value")
                .and_then(Value::as_u64)
                .ok_or_else(|| "original physical law Record identity".into())
        },
    )
}
