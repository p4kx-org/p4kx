// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use super::{Fixture, TestResult, check_coverage, replace_value};

const RECORD_TYPE: &str = "FactionReputation";
const KIND: Kind = Kind::FactionReputations;
const ROW: &str = "/definitions/factionReputations/rows/0";

#[test]
fn faction_reputation_texts_keep_the_historical_display_name_baseline() -> TestResult {
    let fixture = Fixture::from_database(&database())?;
    let (english, german) = fixture.coverage(&fixture.catalog, KIND)?;
    assert_eq!(english.total(), 1, "the original displayName is counted");
    assert_eq!(
        german.resolved, 1,
        "the original German faction name resolves"
    );
    assert_eq!(
        german.supplemental_contradictions, 0,
        "the localized description agrees with the source"
    );

    for (field, form, expected) in [
        ("displayName/value", "displayText", (1, 0)),
        ("description/value/reference", "sourceText", (0, 1)),
    ] {
        let mut catalog = fixture.catalog.clone();
        let reference = fixture
            .catalog
            .pointer(&format!("{ROW}/{field}"))
            .ok_or("actual canonical faction text reference")?;
        let texts = catalog
            .pointer_mut("/texts/gameTexts")
            .and_then(Value::as_array_mut)
            .ok_or("canonical protected text table")?;
        let text = texts
            .iter_mut()
            .find(|target| target.pointer("/id/value") == Some(reference))
            .ok_or("canonical faction text target")?;
        replace_value(
            text,
            &format!("/translations/value/de/{form}/value"),
            json!("Falscher Text"),
        )?;

        let (english, german) = fixture.coverage(&catalog, KIND)?;
        assert_eq!(
            english.total(),
            1,
            "the English historical denominator is preserved"
        );
        assert_eq!(
            german.total(),
            1,
            "the German historical denominator is preserved"
        );
        assert_eq!(
            (german.contradictions, german.supplemental_contradictions),
            expected
        );
        let mut failures = Vec::new();
        check_coverage(KIND, &english, &german, true, 1, &mut failures);
        assert_eq!(
            failures.len(),
            1,
            "a source mismatch independently fails the kind"
        );
    }
    Ok(())
}

#[test]
fn omitting_every_faction_reputation_display_name_cannot_pass_the_baseline() -> TestResult {
    let fixture = Fixture::from_database(&database())?;
    let mut catalog = fixture.catalog.clone();
    replace_value(
        &mut catalog,
        &format!("{ROW}/displayName/state"),
        json!("absent"),
    )?;
    replace_value(
        &mut catalog,
        &format!("{ROW}/displayName/value"),
        Value::Null,
    )?;

    let (english, german) = fixture.coverage(&catalog, KIND)?;
    assert_eq!(english.total(), 0, "the only historical field was removed");
    let mut failures = Vec::new();
    check_coverage(KIND, &english, &german, true, 1, &mut failures);
    assert_eq!(
        failures.len(),
        1,
        "present FactionReputation rows require verifiable display names"
    );
    Ok(())
}

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(RECORD_TYPE)
                .field("displayName", TestType::Locale)
                .field("description", TestType::Locale),
        )
        .record(
            TestRecord::new(
                1,
                RECORD_TYPE,
                "FactionReputation.Fixture",
                "libs/foundry/records/factions/factionreputation/fixture.xml",
            )
            .value("displayName", TestValue::text("@shared"))
            .value("description", TestValue::text("@resource_description")),
        )
}
