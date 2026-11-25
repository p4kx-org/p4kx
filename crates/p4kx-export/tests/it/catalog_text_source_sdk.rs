// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::CatalogLanguageSelection;
use p4kx_export::catalog_queries::localization::{
    SourceIniEncoding, with_language_sources, with_language_sources_with_provenance_mode,
};
use p4kx_export::{CatalogError, CatalogOptions, build_catalog};
use p4kx_extraction::{Archive, CancelToken, Language, SourceValueState};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::{Value, json};

use super::{TestResult, database, english};
use crate::health_origin_assertions as proof;

#[test]
fn language_source_sdk_matches_the_canonical_catalog() -> TestResult {
    let (_directory, archive) = fixture(Some(database()))?;
    let catalog: Value = serde_json::from_slice(
        build_catalog(
            &archive,
            CatalogOptions::new(UNIX_EPOCH).with_full_provenance(),
        )?
        .json(),
    )?;
    with_language_sources_with_provenance_mode(
        &archive,
        CatalogLanguageSelection::AllAvailable,
        full_proof::MODE,
        None,
        |snapshot| -> TestResult {
            assert_eq!(snapshot.sources.len(), Language::ALL.len());
            let scoped = json!({
                "sources": snapshot.sources,
                "provenance": snapshot.source_inventory,
            });
            assert_eq!(
                proof::at(&scoped, "/sources")?,
                proof::at(&catalog, "/texts/languageSources")?,
                "canonical fields and physical source references agree"
            );
            proof::closure(&scoped)
        },
    )??;
    Ok(())
}

#[test]
fn language_source_sdk_leaves_unrequested_payloads_outside_scope() -> TestResult {
    let (_directory, archive) = fixture(None)?;
    let requested = [Language::German, Language::French, Language::German];
    with_language_sources(
        &archive,
        CatalogLanguageSelection::Selected(&requested),
        None,
        |snapshot| -> TestResult {
            assert_eq!(
                snapshot.sources.keys().copied().collect::<Vec<_>>(),
                [Language::German.code(), Language::French.code()]
            );
            let german = snapshot.sources["de"].value().ok_or("German source row")?;
            assert_eq!(
                german.path().value().copied(),
                Some(Language::German.global_ini_path())
            );
            assert_eq!(german.encoding().value(), Some(&SourceIniEncoding::Utf8Bom));
            assert_eq!(german.lossy_decoding().value(), Some(&false));
            assert_eq!(german.skipped_malformed_lines().value(), Some(&1));
            assert_eq!(
                german.encoding().origin().contains_encrypted_source(),
                Some(false)
            );
            let french = &snapshot.sources["fr"];
            assert_eq!(french.state(), SourceValueState::Absent);
            assert!(french.origin().source_set_complete());
            proof::closure(&json!({
                "sources": snapshot.sources,
                "provenance": snapshot.source_inventory,
            }))
        },
    )??;
    assert!(
        with_language_sources(
            &archive,
            CatalogLanguageSelection::AllAvailable,
            None,
            |_| ()
        )
        .is_err(),
        "an explicitly requested encrypted input needs the key"
    );
    Ok(())
}

#[test]
fn language_source_sdk_rechecks_cancellation_after_the_consumer() -> TestResult {
    let (_directory, archive) = fixture(None)?;
    let cancel = CancelToken::new();
    let outcome = with_language_sources(
        &archive,
        CatalogLanguageSelection::Selected(&[Language::German]),
        Some(&cancel),
        |_| cancel.cancel(),
    );
    assert!(matches!(outcome, Err(CatalogError::Cancelled)));
    Ok(())
}

fn fixture(
    dataforge: Option<Vec<u8>>,
) -> Result<(tempfile::TempDir, Archive), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("language-sources.p4k");
    let builder = ArchiveBuilder::new()
        .entry(english().encrypted())
        .entry(TestEntry::stored(
            Language::German.global_ini_path(),
            b"\xef\xbb\xbfname=Deutsch\nmalformed\n",
        ));
    let key = dataforge.as_ref().map(|_| test_key());
    let builder = if let Some(bytes) = dataforge {
        builder.entry(TestEntry::stored("Data\\Game2.dcb", bytes))
    } else {
        builder
    };
    builder.write_to(&path)?;
    Ok((directory, Archive::open_with_key(&path, key)?))
}
