// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::starmap::{
    StarmapDefinitionFamily, StarmapDefinitionRows, StarmapDefinitionSelection,
    StarmapDefinitionSnapshot, with_starmap_definitions,
};
use p4kx_export::catalog_queries::{
    CanonicalRecordHeader, CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
    DatasetAvailability, ParsedCatalogQueries,
};
use p4kx_export::{CatalogError, CatalogOptions, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, Language, SourceLocation};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Value, json};

use super::{Route, TestResult};

#[path = "../it/starmap_definitions_fixture.rs"]
mod fixture;

const OBJECTS: &str = "/definitions/starmapObjects";
const TYPES: &str = "/definitions/starmapTypes";
const PRESETS: &str = "/definitions/starmapPresets";
const AMENITIES: &str = "/definitions/starmapAmenities";
const LANGUAGES: &[Language] = &[Language::English];

pub(in super::super) const DATASETS: [&str; 4] = [OBJECTS, TYPES, PRESETS, AMENITIES];

#[path = "sdk_starmap_object_scope.rs"]
mod objects;

pub(super) fn read() -> TestResult<[Route; 4]> {
    let (_directory, archive) = open_fixture(TestEntry::stored(
        Language::German.global_ini_path(),
        b"shared=Synthetischer Text\n",
    ))?;
    let entry = DataForgeEntry::read(&archive)?;
    let catalog: Value = serde_json::from_slice(
        build_catalog_from(&archive, &entry, &CatalogOptions::new(UNIX_EPOCH))?.json(),
    )?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let parsed = session.parsed(None)?;
    Ok([
        verify_route(&parsed, &catalog, StarmapDefinitionFamily::Objects)?,
        verify_route(&parsed, &catalog, StarmapDefinitionFamily::Types)?,
        verify_route(&parsed, &catalog, StarmapDefinitionFamily::Presets)?,
        verify_route(&parsed, &catalog, StarmapDefinitionFamily::Amenities)?,
    ])
}

fn verify_route(
    parsed: &ParsedCatalogQueries<'_>,
    catalog: &Value,
    family: StarmapDefinitionFamily,
) -> TestResult<Route> {
    parsed.with_starmap_definitions(
        StarmapDefinitionSelection::new(family, CatalogRecordSelection::All),
        None,
        |snapshot| verify_family(catalog, &snapshot),
    )?
}

fn verify_family(catalog: &Value, snapshot: &StarmapDefinitionSnapshot<'_>) -> TestResult<Route> {
    let availability = snapshot.availability;
    match &snapshot.rows {
        StarmapDefinitionRows::Objects(rows) => {
            objects::verify_pu_objects(snapshot, rows)?;
            verify_table(catalog, OBJECTS, availability, rows)
        }
        StarmapDefinitionRows::Types(rows) => {
            let [row] = rows.as_slice() else {
                return Err("one original starmap type".into());
            };
            verify_encrypted_header(row.header());
            assert!(
                row.classification().value().is_some(),
                "the authored public name remains a typed reference"
            );
            verify_table(catalog, TYPES, availability, rows)
        }
        StarmapDefinitionRows::Presets(rows) => {
            let [first, second] = rows.as_slice() else {
                return Err("two physical starmap presets".into());
            };
            verify_encrypted_header(first.header());
            verify_encrypted_header(second.header());
            let mut keys: Vec<_> = snapshot
                .game_texts
                .iter()
                .filter_map(|text| text.key().value().copied())
                .collect();
            keys.sort_unstable();
            assert_eq!(
                keys,
                [fixture::SECOND_PRESET_DESCRIPTION, "@shared"],
                "both authored preset descriptions stay in the protected domain"
            );
            assert!(
                snapshot.public_names.is_empty(),
                "the scoped presets yield no public names"
            );
            verify_table(catalog, PRESETS, availability, rows)
        }
        StarmapDefinitionRows::Amenities(rows) => {
            let [row] = rows.as_slice() else {
                return Err("one original starmap amenity".into());
            };
            verify_encrypted_header(row.header());
            assert!(
                row.required_entity().value().is_some(),
                "the authored location entity remains a typed reference"
            );
            verify_table(catalog, AMENITIES, availability, rows)
        }
        _ => Err("unexpected definition dataset".into()),
    }
}

fn verify_table<Row: JsonSchema + Serialize>(
    catalog: &Value,
    dataset: &'static str,
    availability: DatasetAvailability,
    rows: &[Row],
) -> TestResult<Route> {
    assert_eq!(
        &json!({"availability": availability, "rows": rows}),
        catalog
            .pointer(dataset)
            .ok_or("canonical starmap definition table")?,
        "the scoped rows retain the exact canonical fields and origins"
    );
    Route::queried_rows(dataset, rows, None)
}

fn verify_encrypted_header(header: &CanonicalRecordHeader<'_>) {
    let guid = header.guid();
    assert!(
        guid.origin().source_set_complete(),
        "physical declaration identity has complete source evidence"
    );
    assert_eq!(
        guid.origin().contains_encrypted_source(),
        Some(true),
        "the original declaration identity comes from the encrypted DataForge Entry"
    );
}

#[test]
fn one_starmap_definition_reads_its_owned_fields_and_selected_languages() -> TestResult {
    let german = b"shared=Unused\n";
    let (_directory, archive) = open_fixture(
        TestEntry::stored(Language::German.global_ini_path(), german)
            .with_uncompressed_size(u64::try_from(german.len())? + 1),
    )?;
    let entry = DataForgeEntry::read(&archive)?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(LANGUAGES),
        None,
    )?;
    let parsed = session.parsed(None)?;
    let forge = entry.open()?;
    let preset = forge
        .records()
        .find(|record| {
            record
                .record_path()
                .is_ok_and(|path| path == "selected/preset/first")
        })
        .ok_or("physical preset")?;
    let index = preset.index();
    let records = CatalogRecordSelection::RecordIndex(index);
    let selection = StarmapDefinitionSelection::new(StarmapDefinitionFamily::Presets, records);
    parsed.with_starmap_definitions(selection, None, |snapshot| {
        verify_selected_sources(&snapshot, index)
    })??;
    let all_languages = with_starmap_definitions(
        &archive,
        selection,
        CatalogLanguageSelection::AllAvailable,
        None,
        |_| (),
    );
    assert!(
        matches!(all_languages, Err(CatalogError::LoadTexts(_))),
        "the same request fails when the truncated locale is selected"
    );
    with_starmap_definitions(
        &archive,
        StarmapDefinitionSelection::new(StarmapDefinitionFamily::Amenities, records),
        CatalogLanguageSelection::AllAvailable,
        None,
        |snapshot| {
            let StarmapDefinitionRows::Amenities(rows) = snapshot.rows else {
                return Err("typed empty starmap amenity selection".into());
            };
            assert!(
                rows.is_empty(),
                "the requested physical Record belongs to another family"
            );
            assert!(
                snapshot.available_languages.is_empty(),
                "an empty selection requires no localization payload"
            );
            Ok(())
        },
    )?
}

fn verify_selected_sources(snapshot: &StarmapDefinitionSnapshot<'_>, index: u32) -> TestResult {
    let StarmapDefinitionRows::Presets(rows) = &snapshot.rows else {
        return Err("typed selected starmap preset".into());
    };
    let [row] = rows.as_slice() else {
        return Err("one physical authored starmap preset".into());
    };
    assert_eq!(
        row.header().record_index().value(),
        Some(&index),
        "select the requested physical Record"
    );
    assert_eq!(
        snapshot.available_languages, LANGUAGES,
        "the query reads only the requested English locale"
    );
    let [description] = snapshot.game_texts.as_slice() else {
        return Err("one protected description".into());
    };
    let translations = description
        .translations()
        .value()
        .ok_or("localized description")?;
    assert_eq!(
        translations.len(),
        1,
        "only the selected language is projected"
    );
    let source = row
        .object_type()
        .value()
        .ok_or("authored starmap type target")?;
    assert!(
        source.target_record_index().value().is_some(),
        "the original target header is independently resolved"
    );
    let mut owned_body = false;
    for used in snapshot.source_inventory.iter_uses() {
        if let SourceLocation::Subresource {
            namespace, indices, ..
        } = used.location()
            && namespace == "dataforgeValue"
        {
            assert_eq!(
                indices.first(),
                Some(&u64::from(index)),
                "body evidence belongs to the selected preset"
            );
            owned_body = true;
        }
    }
    assert!(owned_body, "the query retains owned payload field evidence");
    Ok(())
}

fn open_fixture(german: TestEntry) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("starmap-definitions.p4k");
    ArchiveBuilder::new()
        .entry(
            TestEntry::stored(
                "Data/Game2.dcb",
                objects::extend(fixture::database()).build(),
            )
            .encrypted(),
        )
        .entry(TestEntry::stored(
            Language::English.global_ini_path(),
            b"shared=Synthetic description\n",
        ))
        .entry(german)
        .entry(TestEntry::stored(
            "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml",
            b"<LayerBackups/>",
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((folder, archive))
}
