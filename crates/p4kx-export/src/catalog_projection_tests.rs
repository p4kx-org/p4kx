// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_extraction::{Language, SourceUseRole};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::{CatalogProjection, CatalogTextRegistry, Domain, FinishedTexts, Lookup, TextOwner};
use crate::Kind;
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_v3::catalog_text_forms;
use crate::texts::TextProjection;

#[test]
fn placeholder_first_admission_preserves_lookup_proof_and_independent_counts() -> TestResult {
    const KEY: &str = "@LOC_EMPTY";
    let bytes = two_occurrences(KEY);
    for mission_first in [true, false] {
        test_support::with_localized_projection(
            &bytes,
            &[(Language::English, b"other=Available\r\n")],
            |_, projection| {
                if !mission_first {
                    register(projection, KEY, "ordinary")?;
                }
                register_mission_lookup(projection, KEY)?;
                if mission_first {
                    register(projection, KEY, "ordinary")?;
                }
                assert_placeholder_lookup(projection, mission_first)
            },
        )?;
    }
    Ok(())
}

#[test]
fn committed_text_registrations_keep_owned_display_forms_and_exact_use_order() -> TestResult {
    const KEY: &str = "@sample";
    let bytes = two_occurrences(KEY);
    let locales: [(Language, &[u8]); 2] = [
        (Language::English, b"sample=Raw\\nLine\r\n"),
        (Language::German, b"other=Available\r\n"),
    ];
    test_support::with_localized_projection(&bytes, &locales, |_, projection| {
        register(projection, KEY, "ordinary")?;
        let staged = CatalogTextRegistry::default();
        let transaction = projection.with_text_registry(&staged);
        register(&transaction, KEY, "mission")?;
        projection.commit_text_registrations(&transaction)?;
        let counts = projection.legacy_texts().summary();
        let [english, german, ..] = counts.as_slice() else {
            return Err("English and German lookup summaries".into());
        };
        assert_eq!(
            (english.resolved, german.unresolved),
            (2, 2),
            "both actual registrations count exactly once per language"
        );

        let finished = projection.finish()?;
        assert_shared_text_origins(&finished)?;
        assert_text_forms(projection, &finished)?;
        assert_eq!(
            projection.legacy_texts().summary(),
            counts,
            "rebuilding a canonical row does not count another lookup"
        );
        let [english, german, french, ..] = finished.kind_coverage.as_slice() else {
            return Err("English, German, and French coverage summaries".into());
        };
        assert_eq!(
            (
                english.resolved,
                german.missing_key,
                french.missing_language
            ),
            (2, 2, 2),
            "duplicate coverage retains the distinct cause for every language"
        );
        Ok(())
    })
}

fn register(projection: &CatalogProjection<'_, '_>, key: &'static str, field: &str) -> TestResult {
    let source = projection.origins().record(0);
    projection.for_kind(Kind::Items).register_uses(
        Domain::GameText,
        key,
        source.observe(field, SourceUseRole::Payload)?,
        source.observe(field, SourceUseRole::Selector)?,
    )?;
    Ok(())
}

fn register_mission_lookup(
    projection: &CatalogProjection<'_, '_>,
    key: &'static str,
) -> TestResult {
    let source = projection.origins().record(0);
    let mission = projection.for_kind(Kind::Contracts);
    mission.insert_pending(
        Domain::GameText,
        key,
        source.observe("mission", SourceUseRole::Payload)?,
        source.observe("mission", SourceUseRole::Selector)?,
        Lookup::from_projection(&mission.translations(key)),
    )?;
    Ok(())
}

fn assert_placeholder_lookup(
    projection: &CatalogProjection<'_, '_>,
    mission_first: bool,
) -> TestResult {
    let counts = projection.legacy_texts().summary();
    let english = counts.first().ok_or("English legacy counts")?;
    assert_eq!(
        (english.unresolved, english.placeholder),
        (1, 1),
        "the forced lookup and ordinary placeholder have separate counters"
    );
    let finished = projection.finish()?;
    assert_shared_text_origins(&finished)?;
    let row = finished
        .texts
        .game_texts
        .first()
        .ok_or("canonical text row")?;
    let english = row
        .translations
        .value()
        .and_then(|values| values.get("en"))
        .ok_or("loaded language always has a forms entry")?;
    assert_eq!(
        english.source_text.origin().source_uses_refs().len(),
        if mission_first { 4 } else { 2 },
        "the first admission controls whether the locale was consulted"
    );
    let item = finished
        .kind_coverage
        .iter()
        .find(|row| {
            row.owner == TextOwner::Catalog(Kind::Items) && row.language == Language::English
        })
        .ok_or("English item counts")?;
    assert_eq!(
        (item.missing_language, item.placeholder),
        (0, 1),
        "the ordinary registration classifies its own exact key"
    );
    let contract = finished
        .kind_coverage
        .iter()
        .find(|row| {
            row.owner == TextOwner::Catalog(Kind::Contracts) && row.language == Language::English
        })
        .ok_or("English mission counts")?;
    assert_eq!(
        (contract.missing_key, contract.placeholder),
        (1, 0),
        "the mission accounts for its own forced lookup"
    );
    Ok(())
}

fn assert_text_forms(
    projection: &CatalogProjection<'_, '_>,
    finished: &FinishedTexts<'_>,
) -> TestResult {
    let row = finished
        .texts
        .game_texts
        .first()
        .ok_or("canonical text row")?;
    let actual = row
        .translations
        .value()
        .and_then(|values| values.get("en"))
        .ok_or("English translation forms")?;
    assert!(
        matches!(actual.display_text.value(), Some(Cow::Owned(display)) if display == "Raw\nLine"),
        "display retains the exact decoded owned form"
    );
    let origins = projection.origins();
    let source = origins.record(0);
    let selectors = [
        source.observe("ordinary", SourceUseRole::Selector)?,
        source.observe("mission", SourceUseRole::Selector)?,
    ];
    let expected = catalog_text_forms::project(
        &origins.uses,
        origins
            .locale(Language::English)
            .ok_or("English localization")?,
        Language::English,
        &selectors,
        Some(TextProjection {
            source: "Raw\\nLine",
            display: Cow::Borrowed("Raw\nLine"),
        }),
        true,
    )?;
    assert_eq!(
        serde_json::to_vec(actual)?,
        serde_json::to_vec(&expected.forms)?,
        "source and display keep the literal text, per-occurrence origin IDs, order and flags"
    );
    Ok(())
}

fn two_occurrences(key: &str) -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("FixtureTexts")
                .field("mission", TestType::Locale)
                .field("ordinary", TestType::Locale),
        )
        .record(
            TestRecord::new(1, "FixtureTexts", "TextFixture", "fixture/texts.xml")
                .value("mission", TestValue::text(key))
                .value("ordinary", TestValue::text(key)),
        )
        .build()
}

fn assert_shared_text_origins(finished: &FinishedTexts<'_>) -> TestResult {
    let row = finished
        .texts
        .game_texts
        .first()
        .ok_or("canonical text row")?;
    let id = row.id.origin().source_uses_refs();
    let key = row.key.origin().source_uses_refs();
    assert_eq!(id.len(), 2, "both source payloads remain");
    assert!(
        std::ptr::eq(id, key),
        "the derived ID and authored key share the exact contributors"
    );
    Ok(())
}
