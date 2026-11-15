// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Language, SourceUseRole};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::{Domain, TextOwner};
use crate::Kind;
use crate::build::original_input::test_support::{self, TestResult};

#[test]
fn an_existing_lookup_does_not_change_ordinary_placeholder_counts() -> TestResult {
    const KEY: &str = "@LOC_EMPTY";
    let bytes = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("FixtureTexts")
                .field("mission", TestType::Locale)
                .field("ordinary", TestType::Locale),
        )
        .record(
            TestRecord::new(1, "FixtureTexts", "TextFixture", "fixture/texts.xml")
                .value("mission", TestValue::text(KEY))
                .value("ordinary", TestValue::text(KEY)),
        )
        .build();
    test_support::with_projection(&bytes, |_, projection| {
        let source = projection.origins().record(0);
        let mission = projection.for_kind(Kind::Contracts);
        mission.insert_pending(
            Domain::GameText,
            KEY,
            source.observe("mission", SourceUseRole::Payload)?,
            source.observe("mission", SourceUseRole::Selector)?,
            mission.translations(KEY, false),
            true,
        )?;
        projection.for_kind(Kind::Items).register_uses(
            Domain::GameText,
            KEY,
            source.observe("ordinary", SourceUseRole::Payload)?,
            source.observe("ordinary", SourceUseRole::Selector)?,
        )?;

        let legacy = projection.legacy_texts().summary();
        let english = legacy
            .iter()
            .find(|row| row.language == Language::English)
            .ok_or("English legacy counts")?;
        assert_eq!(
            (english.unresolved, english.placeholder),
            (1, 1),
            "only the first registration performed a lookup"
        );
        let finished = projection.finish()?;
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
        Ok(())
    })
}
