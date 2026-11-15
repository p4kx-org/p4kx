// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use super::{Fixture, TestResult, check_coverage, replace_value};

#[test]
fn the_mining_baselines_count_only_original_display_keys() -> TestResult {
    let fixture = Fixture::from_database(&database())?;
    let mut wrong = fixture.catalog.clone();
    let translations = wrong
        .pointer_mut("/texts/gameTexts")
        .and_then(Value::as_array_mut)
        .ok_or("protected text rows")?;
    let translated = translations
        .iter_mut()
        .find(|row| row.pointer("/key/value").and_then(Value::as_str) == Some("@shared"))
        .ok_or("original mining name")?;
    replace_value(
        translated,
        "/translations/value/de/displayText/value",
        json!("Falscher Text"),
    )?;

    for kind in [Kind::RefiningProcesses, Kind::MineableCompositions] {
        let (english, german) = fixture.coverage(&fixture.catalog, kind)?;
        assert_eq!(
            (english.total(), german.total()),
            (2, 2),
            "excluded keys never change the denominators"
        );
        assert_eq!((german.resolved, german.fallback), (1, 1));
        assert_eq!(german.contradictions, 0);

        let (english, german) = fixture.coverage(&wrong, kind)?;
        assert_eq!((english.total(), german.total()), (2, 2));
        assert_eq!(
            german.contradictions, 1,
            "one canonical text mutation fails its historical kind"
        );
        let mut failures = Vec::new();
        check_coverage(kind, &english, &german, false, 4, &mut failures);
        assert_eq!(failures.len(), 1);
    }
    Ok(())
}

fn database() -> DataForgeBuilder {
    let mut builder = DataForgeBuilder::new().version(8);
    for (offset, source, folder, field) in [
        (10_u128, "RefiningProcess", "refiningprocess", "processName"),
        (
            20_u128,
            "MineableComposition",
            "mining/rockcompositionpresets",
            "depositName",
        ),
    ] {
        builder = builder.structure(TestStruct::new(source).field(field, TestType::Locale));
        for (ordinal, key) in [
            (1, "@shared"),
            (2, "@source_missing"),
            (3, ""),
            (4, "@LOC_EMPTY"),
        ] {
            builder = builder.record(
                TestRecord::new(
                    offset + ordinal,
                    source,
                    format!("{source}.Example{ordinal}"),
                    format!("libs/foundry/records/{folder}/mining_example_{ordinal}.xml"),
                )
                .value(field, TestValue::text(key)),
            );
        }
    }
    builder
}
