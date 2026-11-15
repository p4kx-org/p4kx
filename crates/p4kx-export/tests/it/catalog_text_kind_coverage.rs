// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{Kind, Language, TextOwner};

use super::{TestResult, build_fixture, english, german};

#[test]
fn canonical_registration_coverage_preserves_legacy_resolution_counts() -> TestResult {
    let built = build_fixture(vec![english(), german()])?;
    let summary = built.summary();
    let owner = TextOwner::Catalog(Kind::Manufacturers);
    let coverage = summary.text_kind_coverage();
    assert_eq!(
        coverage.len(),
        Language::ALL.len(),
        "one owner and all supported languages"
    );
    assert!(
        coverage.iter().all(|row| row.owner == owner),
        "the registered owner is Manufacturers"
    );

    let german = coverage
        .iter()
        .find(|row| row.language == Language::German)
        .ok_or("missing German coverage")?;
    let legacy = summary
        .text(Language::German)
        .ok_or("missing German summary")?;
    assert_eq!(
        (german.resolved, german.missing_language, german.missing_key),
        (2, 0, 2),
        "only the exact present keys are resolved"
    );
    assert_eq!(
        (
            german.empty_source,
            german.empty_display,
            german.placeholder
        ),
        (1, 0, 1),
        "an empty winner and a recognized placeholder are distinct"
    );
    assert_eq!(
        (legacy.resolved, legacy.unresolved, legacy.placeholder),
        (
            german.resolved,
            german.missing_key + german.empty_source,
            german.placeholder
        ),
        "the finer categories preserve legacy totals"
    );
    let french = coverage
        .iter()
        .find(|row| row.language == Language::French)
        .ok_or("missing French coverage")?;
    assert_eq!(
        (french.missing_language, french.placeholder),
        (5, 1),
        "unavailable French is not a missing key"
    );
    Ok(())
}
