// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use super::{KindTextCoverage, KindTextSummary, Lookup, Outcome, TextOwner, Translations};
use crate::Kind;
use crate::texts::{CATALOG_LANGUAGES, TextProjection};
use p4kx_extraction::Language;

#[test]
fn lookup_outcomes_distinguish_source_gaps_from_present_empty_values() {
    let translations = Translations::from([
        (Language::English, Some(text("display", "display"))),
        (Language::German, None),
        (Language::French, Some(text("", ""))),
        (Language::Italian, Some(text("  ", ""))),
    ]);
    let lookup = Lookup::from_projection(&translations);

    let expected = [
        (Language::English, Outcome::Resolved),
        (Language::German, Outcome::MissingKey),
        (Language::French, Outcome::EmptySource),
        (Language::Italian, Outcome::EmptyDisplay),
        (Language::SpanishSpain, Outcome::MissingLanguage),
    ];
    for (language, expected_outcome) in expected {
        assert_eq!(
            outcome(lookup, language),
            Some(expected_outcome),
            "lookup for {language:?}"
        );
    }
    assert_eq!(
        outcome(Lookup::Placeholder, Language::SpanishSpain),
        Some(Outcome::Placeholder),
        "a placeholder never attempts to look up a missing language"
    );
}

#[test]
fn coverage_counts_registrations_in_stable_owner_and_language_order() -> Result<(), &'static str> {
    let mut counts = KindTextCoverage::default();
    let translations = Translations::from([(Language::English, Some(text("name", "name")))]);
    let ships = TextOwner::Catalog(Kind::Ships);
    let items = TextOwner::Catalog(Kind::Items);

    counts.record(
        TextOwner::ConsumableSubtypes,
        Lookup::from_projection(&translations),
    );
    counts.record(ships, Lookup::from_projection(&translations));
    counts.record(items, Lookup::from_projection(&translations));
    counts.record(items, Lookup::from_projection(&translations));
    counts.record(items, Lookup::Placeholder);
    counts.record(
        TextOwner::ItemRecoveryPolicy,
        Lookup::from_projection(&translations),
    );

    let result = counts.finish();
    assert_eq!(
        result.len(),
        4 * Language::ALL.len(),
        "all supported languages for each owner"
    );
    let (owner_groups, _) = result.as_chunks::<{ Language::ALL.len() }>();
    for (group, owner) in owner_groups.iter().zip([
        items,
        ships,
        TextOwner::ItemRecoveryPolicy,
        TextOwner::ConsumableSubtypes,
    ]) {
        for (row, language) in group.iter().zip(Language::ALL) {
            assert_eq!(
                (row.owner, row.language),
                (owner, language),
                "stable owner and language order"
            );
        }
    }
    let english_item = result.first().ok_or("no first coverage row")?;
    assert_eq!(
        (english_item.resolved, english_item.placeholder),
        (2, 1),
        "repeated successful and placeholder registrations have separate counts"
    );
    let german = result
        .iter()
        .find(|row| (row.owner, row.language) == (items, Language::German))
        .ok_or("no German item coverage row")?;
    assert_eq!(
        (german.missing_language, german.placeholder),
        (2, 1),
        "an unavailable language keeps placeholders separate"
    );
    assert_eq!(
        registrations(german),
        3,
        "one disjoint outcome per registration"
    );
    Ok(())
}

fn outcome(lookup: Lookup, language: Language) -> Option<Outcome> {
    CATALOG_LANGUAGES
        .into_iter()
        .zip(lookup.outcomes())
        .find_map(|(candidate, outcome)| (language == candidate).then_some(outcome))
}

const fn text<'source>(source: &'source str, display: &'source str) -> TextProjection<'source> {
    TextProjection {
        source,
        display: Cow::Borrowed(display),
    }
}

const fn registrations(row: &KindTextSummary) -> usize {
    row.resolved
        + row.missing_language
        + row.missing_key
        + row.empty_source
        + row.empty_display
        + row.placeholder
}
