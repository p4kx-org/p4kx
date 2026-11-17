// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, DisplayTransformationPayload,
    GameplayPropertySnapshot, read_gameplay_properties, read_gameplay_properties_from,
};
use p4kx_export::{CatalogError, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, FieldOrigin, Language, LocalizationNote};
use p4kx_testkit::{ArchiveBuilder, TestEntry, TestValue};
use serde_json::Value;

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::{gameplay_v3_support as support, gameplay_v3_values as values};

#[test]
fn selected_gameplay_fields_match_catalog_values_and_original_source_uses() -> TestResult {
    let (_folder, archive) = common::open(&values::simple(), DcbOrigin::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let snapshot = read_gameplay_properties_from(
        &archive,
        &entry,
        CatalogRecordSelection::All,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let full = build_catalog_from(&archive, &entry, &common::options())?;
    let catalog: Value = serde_json::from_slice(full.json())?;
    assert_eq!(snapshot.rows.len(), 1);
    let (row, expected) = (&snapshot.rows[0], support::row(&catalog, 0)?);
    assert_eq!(&serde_json::to_value(row)?, expected);
    let transform = row
        .fields
        .display_transformation
        .value()
        .ok_or("transformation")?;
    let node = &transform.nodes[0];
    let Some(DisplayTransformationPayload::Scale { scale }) = node.payload.value() else {
        return Err("source scale transformation".into());
    };
    let key = row
        .fields
        .property_name
        .value()
        .ok_or("original public name reference")?;
    let text = snapshot
        .public_names
        .iter()
        .find(|text| {
            text.id
                .value()
                .is_some_and(|candidate| candidate.as_str() == key.as_str())
        })
        .ok_or("public-name definition referenced by the gameplay field")?;
    let translated = text.translations.value().ok_or("selected translations")?;
    assert_eq!(
        translated["de"].source_text.value().map(AsRef::as_ref),
        Some("Ressource Deutsch")
    );
    assert_eq!(
        translated["en"].display_text.value().map(AsRef::as_ref),
        Some("Resource English")
    );
    assert_eq!(
        &serde_json::to_value(text)?,
        common::text_row(&catalog, "publicNames", &expected["propertyName"]["value"])?
    );
    assert_eq!(snapshot.localization_notes, [] as [LocalizationNote; 0]);
    assert_eq!(snapshot.coverage.incomplete_fields, [] as [u32; 0]);
    assert_field_origins(
        &snapshot,
        [scale.origin(), translated["en"].source_text.origin()],
    );
    Ok(())
}

fn assert_field_origins(snapshot: &GameplayPropertySnapshot, origins: [&FieldOrigin; 2]) {
    for origin in origins {
        assert_eq!(
            origin.contains_encrypted_source(),
            Some(true),
            "the field uses the encrypted source"
        );
        assert_ne!(
            origin.source_uses_refs(),
            [],
            "the field retains its source-use references"
        );
        for reference in origin.source_uses_refs() {
            assert!(
                snapshot
                    .source_inventory
                    .iter_uses()
                    .any(|source| source.id() == reference),
                "the query inventory resolves each retained source use"
            );
        }
    }
}

fn localization_selection_archive() -> TestResult<(tempfile::TempDir, Archive)> {
    let database = values::simple().record(values::record(
        values::FIRST,
        "Second",
        values::scale(TestValue::Single(2.5)),
    ));
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("fixture.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()))
        .entry(
            TestEntry::stored(
                Language::English.global_ini_path(),
                b"resource_name=Locked\n",
            )
            .encrypted(),
        )
        .entry(TestEntry::stored(
            Language::German.global_ini_path(),
            b"resource_name=Ressource\n",
        ))
        .write_to(&path)?;
    Ok((folder, Archive::open(path)?))
}

#[test]
fn gameplay_queries_keep_physical_aliases_and_skip_unselected_languages() -> TestResult {
    let (_folder, archive) = localization_selection_archive()?;
    let entry = DataForgeEntry::read(&archive)?;
    let guid = entry.open()?.record(0).ok_or("first Record")?.guid();
    assert!(matches!(
        read_gameplay_properties_from(
            &archive,
            &entry,
            CatalogRecordSelection::RecordIndex(1),
            CatalogLanguageSelection::AllAvailable,
            None
        ),
        Err(CatalogError::LoadTexts(_))
    ));
    let subset = CatalogLanguageSelection::Selected(&[Language::French, Language::German]);
    let by_guid = read_gameplay_properties_from(
        &archive,
        &entry,
        CatalogRecordSelection::Guid(guid),
        subset,
        None,
    )?;
    let physical: Vec<_> = by_guid
        .rows
        .iter()
        .filter_map(|row| row.source.record_index.value().copied())
        .collect();
    assert_eq!(physical, [0, 1]);
    assert_eq!(
        by_guid.requested_languages,
        [Language::German, Language::French]
    );
    assert_eq!(by_guid.available_languages, [Language::German]);
    let exact = read_gameplay_properties(
        &archive,
        CatalogRecordSelection::RecordIndex(1),
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?;
    assert_eq!(exact.rows.len(), 1);
    assert_eq!(exact.rows[0].source.record_index.value(), Some(&1));
    assert!(!exact.public_names.is_empty());
    for name in &exact.public_names {
        assert!(
            name.translations
                .value()
                .ok_or("translation map")?
                .is_empty()
        );
    }
    Ok(())
}
