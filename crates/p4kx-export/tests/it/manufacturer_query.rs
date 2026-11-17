// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ManufacturerSnapshot,
    with_manufacturers,
};
use p4kx_export::{CatalogError, build_catalog};
use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, FieldOrigin, Language};
use p4kx_testkit::{ArchiveBuilder, TestEntry, TestRecord, TestValue};
use serde_json::Value;

use crate::commodity_v3_fixture::{self as archive_fixture, DcbOrigin};
use crate::manufacturer_v3_fixture::{self as fixture, TestResult};

#[test]
fn manufacturers_match_the_catalog_and_resolve_both_text_domains() -> TestResult {
    let database = fixture::database().record(
        fixture::record(
            0x42,
            "NoTextMaker",
            "libs/foundry/records/scitemmanufacturer/included/other.xml",
        )
        .value("Code", TestValue::text("OTHER"))
        .value("Localization", TestValue::Null),
    );
    let (_folder, archive) = archive_fixture::open(&database, DcbOrigin::Plain)?;
    let built = build_catalog(&archive, &archive_fixture::options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let entry = DataForgeEntry::read(&archive)?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let parsed = session.parsed(None)?;
    parsed.with_manufacturers(CatalogRecordSelection::All, None, |snapshot| {
        assert_eq!(
            serde_json::to_value(&snapshot.rows)?,
            Value::Array(fixture::rows(&catalog)?.clone())
        );
        assert_eq!(
            serde_json::to_value(snapshot.availability)?,
            *fixture::at(&catalog, "/definitions/manufacturers/availability")?
        );
        verify_texts(&snapshot, &catalog)
    })??;

    let guid = fixture::guid(fixture::MAKER).parse()?;
    for selection in [
        CatalogRecordSelection::Guid(guid),
        CatalogRecordSelection::RecordIndex(0),
    ] {
        parsed.with_manufacturers(selection, None, |snapshot| {
            let [row] = snapshot.rows.as_slice() else {
                return Err("one manufacturer Record".into());
            };
            assert_eq!(row.source().guid().value(), Some(&guid));
            assert_eq!(row.source().record_index().value(), Some(&0));
            assert_eq!(&serde_json::to_value(row)?, fixture::row(&catalog)?);
            verify_texts(&snapshot, &catalog)
        })??;
    }
    Ok(())
}

fn verify_texts(snapshot: &ManufacturerSnapshot<'_>, catalog: &Value) -> TestResult {
    let owner = snapshot.rows.first().ok_or("manufacturer owner")?;
    let localization = owner.localization().value().ok_or("localization")?;
    let public_id = localization.name().value().ok_or("public name")?;
    let protected_id = localization.description().value().ok_or("protected text")?;
    let public = snapshot.public_name(public_id).ok_or("public name row")?;
    let protected = snapshot
        .game_text(protected_id)
        .ok_or("protected text row")?;
    assert_ne!(
        public_id.as_str(),
        protected_id.as_str(),
        "the text domains have separate identifiers"
    );
    assert_eq!(
        public.key().value(),
        protected.key().value(),
        "one original key can occur in both domains"
    );
    assert_eq!(
        &serde_json::to_value(public)?,
        fixture::text(catalog, "publicNames", "name")?,
        "the selected public row matches the Catalog"
    );
    assert_eq!(
        &serde_json::to_value(protected)?,
        fixture::text(catalog, "gameTexts", "description")?,
        "the selected protected row matches the Catalog"
    );
    let translations = public.translations().value().ok_or("translations")?;
    let english = translations.get("en").ok_or("English text")?;
    let german = translations.get("de").ok_or("German text")?;
    assert_eq!(
        english.source_text.origin().contains_encrypted_source(),
        Some(true),
        "English came from the encrypted Entry"
    );
    assert_eq!(
        german.source_text.origin().contains_encrypted_source(),
        Some(false),
        "German came from the clear Entry"
    );
    assert_source_use_closure(
        snapshot,
        [
            localization.name().origin(),
            protected.key().origin(),
            english.source_text.origin(),
        ],
    );
    Ok(())
}

fn assert_source_use_closure(snapshot: &ManufacturerSnapshot<'_>, origins: [&FieldOrigin; 3]) {
    for origin in origins {
        assert_ne!(
            origin.source_uses_refs(),
            [],
            "each selected value retains its source references"
        );
        for reference in origin.source_uses_refs() {
            assert!(
                snapshot
                    .source_inventory
                    .iter_uses()
                    .any(|item| item.id() == reference),
                "the selected result resolves each retained source use"
            );
        }
    }
}

#[test]
fn manufacturer_guids_keep_physical_aliases() -> TestResult {
    let database = fixture::database().record(
        fixture::record(
            fixture::MAKER,
            "Alias",
            "libs/foundry/records/scitemmanufacturer/included/alias.xml",
        )
        .value("Code", TestValue::text("ALIAS"))
        .value("Localization", TestValue::Null),
    );
    let (_folder, archive) = archive_fixture::open(&database, DcbOrigin::Plain)?;
    let guid = fixture::guid(fixture::MAKER).parse()?;
    let positions = with_manufacturers(
        &archive,
        CatalogRecordSelection::Guid(guid),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            snapshot
                .rows
                .iter()
                .filter_map(|row| row.source().record_index().value().copied())
                .collect::<Vec<_>>()
        },
    )?;
    assert_eq!(positions, [0, 1]);
    Ok(())
}

#[test]
fn out_of_family_manufacturer_lookup_leaves_locked_locales_unread() -> TestResult {
    let (_folder, archive) = locked_locale_archive()?;
    let session = CatalogQuerySession::open(
        &archive,
        CatalogLanguageSelection::Selected(&[Language::English]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    for selection in [
        CatalogRecordSelection::RecordIndex(1),
        CatalogRecordSelection::RecordIndex(u32::MAX),
    ] {
        parsed.with_manufacturers(selection, None, |snapshot| {
            assert!(snapshot.rows.is_empty());
            assert_eq!(
                snapshot.available_languages,
                [],
                "an empty selection never opens locale Entries"
            );
            assert_eq!(snapshot.requested_languages, [Language::English]);
        })?;
    }
    assert_eq!(archive.key_demand().snapshot().unread(), 0);
    assert!(matches!(
        parsed.with_manufacturers(CatalogRecordSelection::RecordIndex(0), None, |_| ()),
        Err(CatalogError::LoadTexts(_))
    ));
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(matches!(
        parsed.with_manufacturers(CatalogRecordSelection::All, Some(&cancel), |_| panic!(
            "cancelled query"
        )),
        Err(CatalogError::Cancelled)
    ));
    Ok(())
}

#[test]
fn manufacturer_queries_retain_encrypted_and_unknown_dataforge_origins() -> TestResult {
    let database = fixture::database();
    for (origin, expected) in [
        (DcbOrigin::Encrypted, Some(true)),
        (DcbOrigin::Unknown, None),
    ] {
        let (_folder, archive) = archive_fixture::open(&database, origin)?;
        with_manufacturers(
            &archive,
            CatalogRecordSelection::RecordIndex(0),
            CatalogLanguageSelection::Selected(&[]),
            None,
            |snapshot| {
                let [row] = snapshot.rows.as_slice() else {
                    return Err("one manufacturer Record".into());
                };
                assert_eq!(row.code().origin().contains_encrypted_source(), expected);
                Ok::<(), Box<dyn std::error::Error>>(())
            },
        )??;
    }
    Ok(())
}

fn locked_locale_archive() -> TestResult<(tempfile::TempDir, Archive)> {
    let database = fixture::database().record(TestRecord::new(
        0x53,
        "Empty",
        "Empty.Unrelated",
        "libs/foundry/records/unrelated.xml",
    ));
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("fixture.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()))
        .entry(
            TestEntry::stored(Language::English.global_ini_path(), b"shared=Locked\n").encrypted(),
        )
        .write_to(&path)?;
    Ok((folder, Archive::open(path)?))
}
