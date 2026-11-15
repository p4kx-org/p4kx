// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use serde_json::{Value, json};

use super::{Oracle, TestResult, real_archive_canonical_texts, replace_value};

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
    let coverage = |catalog: &Value| {
        real_archive_canonical_texts::kind_coverage(
            catalog,
            Kind::MissionPropertySets,
            &[],
            &english,
            &german,
        )
    };
    let (en, de) = coverage(&catalog)?;
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
        coverage(&catalog).is_err(),
        "a keyed option cannot vanish from coverage"
    );
    Ok(())
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
