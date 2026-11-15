// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_export::Kind;
use serde_json::{Value, json};

use super::{Coverage, Oracle, TestResult, real_archive_canonical_texts, replace_value};

#[test]
fn generator_option_text_coverage_counts_every_authored_occurrence() -> TestResult {
    let english = Oracle::parse("option=Selected");
    let german = Oracle::parse("option=Gewählt");
    let mut catalog = json!({
        "texts": {
            "publicNames": [],
            "gameTexts": [{
                "id": present(&json!("shared")),
                "key": present(&json!("@option")),
                "translations": present(&json!({
                    "en": forms("Selected"), "de": forms("Gewählt")
                }))
            }]
        },
        "missionPropertySets": [{
            "properties": [
                null,
                {"value": {"type": "integer", "options": [
                    {"textId": "@option", "displayName": present(&json!("shared"))},
                    null,
                    {"textId": "@LOC_UNINITIALIZED", "displayName": absent()}
                ]}},
                {"value": {"type": "float", "options": [
                    {"textId": "@option", "displayName": present(&json!("shared"))}
                ]}},
                {"value": {"type": "tags", "options": null}}
            ]
        }]
    });
    let (en, de) = mission_coverage(&catalog, &english, &german)?;
    assert_eq!(
        [en.total(), de.total()],
        [2, 2],
        "repeated references count per option"
    );
    assert_eq!(
        [en.resolved, de.resolved],
        [2, 2],
        "both original languages are compared"
    );

    replace_value(
        &mut catalog,
        "/missionPropertySets/0/properties/2/value/options/0/displayName",
        absent(),
    )?;
    assert!(
        mission_coverage(&catalog, &english, &german).is_err(),
        "a keyed option cannot vanish from coverage"
    );
    Ok(())
}

#[test]
fn generator_investigation_text_coverage_keeps_historical_locale_tokens() -> TestResult {
    let english = Oracle::parse("bare=Read body\ncause=Cause\nLOC_PLACEHOLDER=Ignored");
    let german = Oracle::parse("bare=Koerper lesen\ncause=Ursache\nLOC_PLACEHOLDER=Ignoriert");
    let mut catalog = investigation_catalog();
    let (en, de) = mission_coverage(&catalog, &english, &german)?;
    assert_eq!(
        [en.total(), de.total()],
        [4, 4],
        "each non-placeholder occurrence counts"
    );
    assert_eq!(
        [en.resolved, de.resolved],
        [3, 3],
        "bare keys resolve per occurrence"
    );
    assert_eq!(
        [en.fallback, de.fallback],
        [1, 1],
        "the empty key counts as a fallback"
    );
    assert_eq!(
        [en.contradictions, de.contradictions],
        [0, 0],
        "both language forms agree"
    );

    replace_value(
        &mut catalog,
        "/texts/gameTexts/3/translations/value/de/sourceText",
        present(&json!("Incorrect")),
    )?;
    let (en, de) = mission_coverage(&catalog, &english, &german)?;
    assert_eq!(
        [en.contradictions, de.contradictions],
        [0, 1],
        "the corpse-scan cause retains independent German source verification",
    );
    Ok(())
}

fn investigation_catalog() -> Value {
    let absent = absent();
    let no_forms = json!({"sourceText": absent, "displayText": absent});
    json!({
        "texts": {
            "publicNames": [],
            "gameTexts": [
                game_text("bare", "bare", &forms("Read body"), &forms("Koerper lesen")),
                game_text("empty", "", &no_forms, &no_forms),
                game_text("placeholder", "@LOC_PLACEHOLDER", &no_forms, &no_forms),
                game_text("cause", "@cause", &forms("Cause"), &forms("Ursache"))
            ]
        },
        "missionPropertySets": [{
            "properties": [
                {"value": {"type": "investigation", "locations": null}},
                {"value": {"type": "investigation", "locations": [
                    null,
                    {"clueItems": null},
                    {"clueItems": [
                        null,
                        {"clues": null},
                        {"clues": [
                            null,
                            {
                                "objectiveLongDescription": present(&json!("bare")),
                                "objectiveShortDescription": present(&json!("empty")),
                                "details": {"type": "corpseScanInfo", "causeOfDeath": present(&json!("cause"))}
                            },
                            {
                                "objectiveLongDescription": present(&json!("bare")),
                                "objectiveShortDescription": present(&json!("placeholder")),
                                "details": {"type": "base"}
                            }
                        ]}
                    ]}
                ]}}
            ]
        }]
    })
}

fn game_text(id: &str, key: &str, english: &Value, german: &Value) -> Value {
    json!({
        "id": present(&json!(id)),
        "key": present(&json!(key)),
        "translations": present(&json!({"en": english, "de": german}))
    })
}

fn mission_coverage(
    catalog: &Value,
    english: &Oracle,
    german: &Oracle,
) -> Result<(Coverage, Coverage), Box<dyn Error>> {
    real_archive_canonical_texts::kind_coverage(
        catalog,
        Kind::MissionPropertySets,
        &[],
        english,
        german,
    )
}

fn present(value: &Value) -> Value {
    json!({
        "state": "present", "value": value, "containsEncryptedSource": false,
        "sourceUsesRefs": [], "sourceSetComplete": true,
    })
}

fn absent() -> Value {
    json!({
        "state": "absent", "value": null, "containsEncryptedSource": false,
        "sourceUsesRefs": [], "sourceSetComplete": true,
    })
}

fn forms(text: &str) -> Value {
    json!({"sourceText": present(&json!(text)), "displayText": present(&json!(text))})
}
