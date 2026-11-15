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

#[test]
fn generator_ai_name_coverage_checks_each_protected_name_occurrence() -> TestResult {
    let english = Oracle::parse("known=Known");
    let german = Oracle::parse("known=Bekannt");
    let mut catalog = ai_name_catalog();
    let (en, de) = mission_coverage(&catalog, &english, &german)?;
    assert_eq!(
        [en.resolved, de.resolved],
        [2, 2],
        "a shared registry row still counts per name field",
    );
    assert_eq!(
        [en.fallback, de.fallback],
        [1, 1],
        "an admitted key without a translation still counts",
    );
    assert_eq!(
        [en.contradictions, de.contradictions],
        [0, 0],
        "both languages match the independent INI",
    );
    replace_value(
        &mut catalog,
        "/missionPropertySets/0/properties/0/value/givenNickName",
        present(&json!("broken")),
    )?;
    assert!(
        mission_coverage(&catalog, &english, &german).is_err(),
        "a broken name join fails the coverage check",
    );
    Ok(())
}

fn ai_name_catalog() -> Value {
    let empty_forms = json!({"sourceText": absent(), "displayText": absent()});
    json!({
        "texts": {
            "publicNames": [],
            "gameTexts": [
                {
                    "id": present(&json!("shared")),
                    "key": present(&json!("@known")),
                    "translations": present(&json!({
                        "en": forms("Known"), "de": forms("Bekannt")
                    }))
                },
                {
                    "id": present(&json!("missing")),
                    "key": present(&json!("@missing")),
                    "translations": present(&json!({"en": empty_forms, "de": empty_forms}))
                }
            ]
        },
        "missionPropertySets": [{
            "properties": [
                {"value": {
                    "type": "aiName",
                    "givenName": present(&json!("shared")),
                    "givenLastName": absent(),
                    "givenNickName": present(&json!("shared"))
                }},
                {"value": {
                    "type": "aiName",
                    "givenName": present(&json!("missing")),
                    "givenLastName": absent(),
                    "givenNickName": absent()
                }}
            ]
        }]
    })
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
                {"value": {"type": "investigation", "suspects": null, "locations": null}},
                {"value": {"type": "investigation", "suspects": null, "locations": [
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
                                "details": {"type": "corpseScanInfo", "causeOfDeath": present(&json!("cause")), "name": null}
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

#[test]
fn investigation_name_coverage_checks_each_original_key_and_language_occurrence() -> TestResult {
    let english = Oracle::parse("shared=Shared name\nfamily=Family");
    let german = Oracle::parse("shared=Gemeinsamer Name\nfamily=Familie");
    let catalog = investigation_name_catalog();
    let (en, de) = mission_coverage(&catalog, &english, &german)?;
    assert_eq!(
        [en.resolved, de.resolved],
        [3, 3],
        "the repeated suspect and corpse name each count alongside the family name",
    );
    assert_eq!(
        [en.fallback, de.fallback],
        [1, 1],
        "a missing translation counts while bare and placeholder tokens do not",
    );
    assert_eq!(
        [en.contradictions, de.contradictions],
        [0, 0],
        "both independent language forms agree",
    );

    let suspect = "/missionPropertySets/0/properties/0/value/suspects/2/name";
    let corpse =
        "/missionPropertySets/0/properties/0/value/locations/0/clueItems/0/clues/0/details/name";
    for (path, value) in [
        (format!("{suspect}/givenName"), present(&json!("family"))),
        (format!("{corpse}/givenName"), absent()),
        (format!("{corpse}/givenNickName"), present(&json!("shared"))),
    ] {
        let mut changed = catalog.clone();
        replace_value(&mut changed, &path, value)?;
        assert!(
            mission_coverage(&changed, &english, &german).is_err(),
            "a valid but wrong, missing or ineligible target cannot count for {path}",
        );
    }
    let mut changed = catalog;
    replace_value(
        &mut changed,
        "/texts/gameTexts/0/translations/value/de/sourceText",
        present(&json!("Incorrect")),
    )?;
    let (en, de) = mission_coverage(&changed, &english, &german)?;
    assert_eq!(
        [en.contradictions, de.contradictions],
        [0, 2],
        "the source text mutation is counted separately for both German name occurrences",
    );
    Ok(())
}

fn investigation_name_catalog() -> Value {
    let no_forms = json!({"sourceText": absent(), "displayText": absent()});
    json!({
        "texts": {
            "publicNames": [],
            "gameTexts": [
                game_text("shared", "@Shared", &forms("Shared name"), &forms("Gemeinsamer Name")),
                game_text("family", "@family", &forms("Family"), &forms("Familie")),
                game_text("missing", "@missing", &no_forms, &no_forms)
            ]
        },
        "missionPropertySets": [{
            "properties": [{"value": {
                "type": "investigation",
                "suspects": [null, {"name": null}, {"name": {
                    "givenName": present(&json!("shared")),
                    "givenLastName": present(&json!("family")),
                    "givenNickName": absent(),
                    "source": {
                        "characterGivenName": "@Shared",
                        "characterGivenLastName": "@family",
                        "characterGivenNickName": "bare"
                    }
                }}],
                "locations": [{"clueItems": [{"clues": [{
                    "objectiveLongDescription": absent(),
                    "objectiveShortDescription": absent(),
                    "details": {"type": "corpseScanInfo", "causeOfDeath": absent(), "name": {
                        "givenName": present(&json!("shared")),
                        "givenLastName": present(&json!("missing")),
                        "givenNickName": absent(),
                        "source": {
                            "characterGivenName": "@Shared",
                            "characterGivenLastName": "@missing",
                            "characterGivenNickName": "@LOC_PLACEHOLDER"
                        }
                    }}
                }]}]}]
            }}]
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
