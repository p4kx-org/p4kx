// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::catalog_queries::equipment;
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{GlobalIniBuilder, TestEntry};

use super::fixture;
use crate::catalog_fixture::{self as catalog_fixture, Maker};
use crate::catalog_items::definitions::equipment_sdk::walk;
use crate::catalog_ship_base::source_sdk_walk;
use crate::commodity_v3_fixture::TestResult;
use crate::health_origin_assertions as proof;

#[test]
fn direct_ship_sdk_manufacturer_storage_and_text_resolvers_match_canonical() -> TestResult {
    for maker in [Maker::Reference, Maker::StrongPointer, Maker::WeakPointer] {
        let folder = tempfile::tempdir()?;
        let locales = [catalog_fixture::english(), german()];
        let path = catalog_fixture::write_archive_with(folder.path(), &locales, maker)?;
        let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
        let catalog = fixture::catalog(&archive)?;
        full_proof::open(
            &archive,
            CatalogLanguageSelection::Selected(&[Language::English, Language::German]),
            None,
        )?
        .with_ship_sources(CatalogRecordSelection::All, None, |snapshot| {
            assert!(!snapshot.rows().is_empty(), "positive original Ship family");
            for row in snapshot.rows() {
                let index = *row
                    .source()
                    .record_index()
                    .value()
                    .ok_or("physical Ship index")?;
                let canonical = proof::rows(&catalog, "/ships")?
                    .iter()
                    .find(|row| {
                        row.pointer("/recordIndex/value")
                            .and_then(serde_json::Value::as_u64)
                            == Some(u64::from(index))
                    })
                    .ok_or("canonical physical Ship")?;
                source_sdk_walk::row(row, snapshot.source_inventory(), canonical)?;
            }
            verify_texts(&snapshot, &catalog)
        })??;
    }
    Ok(())
}

fn verify_texts(
    snapshot: &equipment::ShipSourceSnapshot<'_>,
    catalog: &serde_json::Value,
) -> TestResult {
    for language in [Language::English, Language::German] {
        assert!(
            snapshot.requested_languages().contains(&language),
            "retain requested locale"
        );
        assert!(
            snapshot.available_languages().contains(&language),
            "retain actually read locale"
        );
    }
    assert!(
        !snapshot.public_names().is_empty(),
        "positive public text definitions are emitted"
    );
    assert!(
        !snapshot.game_texts().is_empty(),
        "protected descriptions retain a separate domain"
    );
    for text in snapshot.public_names() {
        let id = text.id().value().ok_or("public name identity")?;
        assert!(
            snapshot
                .public_name(id)
                .is_some_and(|resolved| std::ptr::eq(resolved, text)),
            "public resolver returns this same snapshot's definition"
        );
        walk::text(text, snapshot.source_inventory(), catalog, "publicNames")?;
    }
    for text in snapshot.game_texts() {
        let id = text.id().value().ok_or("protected text identity")?;
        assert!(
            snapshot
                .game_text(id)
                .is_some_and(|resolved| std::ptr::eq(resolved, text)),
            "protected resolver returns this same snapshot's definition"
        );
        walk::text(text, snapshot.source_inventory(), catalog, "gameTexts")?;
    }
    Ok(())
}

fn german() -> TestEntry {
    TestEntry::stored(
        Language::German.global_ini_path(),
        GlobalIniBuilder::new()
            .text("vehicle_NameACAS_Ship", "Frachtschiff")
            .text("vehicle_focus_transporter", "Transporter")
            .build(),
    )
}
