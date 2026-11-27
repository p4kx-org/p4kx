// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_extraction::{SourceUses, SourceValue};
use serde_json::{Value, json};

use super::{Fixture, TestResult, replace_value};

#[test]
fn location_template_baseline_counts_each_variant_using_a_shared_text() -> TestResult {
    let mut fixture = Fixture::new()?;
    let index = fixture.catalog["texts"]["gameTexts"]
        .as_array()
        .ok_or("protected texts")?
        .iter()
        .position(|row| row["key"]["value"] == "@shared")
        .ok_or("fixture texts")?;
    let shared = &fixture.catalog["texts"]["gameTexts"][index]["id"]["value"];
    let registry = SourceUses::new();
    let reference = registry.value(SourceValue::Present(shared.clone()), &[], true);
    let absent = registry.value(SourceValue::<Value>::Absent, &[], true);
    let names = registry.value(
        SourceValue::Present(vec![
            registry.value(
                SourceValue::Present(json!({"displayName": reference})),
                &[],
                true,
            ),
            registry.value(
                SourceValue::Present(json!({"displayName": absent})),
                &[],
                true,
            ),
            registry.value(
                SourceValue::Present(json!({"displayName": reference})),
                &[],
                true,
            ),
        ]),
        &[],
        true,
    );
    fixture.catalog["missionLocationTemplates"] = json!([{"names": names}]);

    let kind = Kind::MissionLocationTemplates;
    let (english, german) = fixture.coverage(&fixture.catalog, kind)?;
    assert_eq!((english.total(), german.total()), (2, 2));
    assert_eq!((english.resolved, german.resolved), (2, 2));

    let mut wrong = fixture.catalog.clone();
    replace_value(
        &mut wrong,
        &format!("/texts/gameTexts/{index}/translations/value/de/displayText/value"),
        json!("Falscher Name"),
    )?;
    let (english, german) = fixture.coverage(&wrong, kind)?;
    assert_eq!((english.total(), german.total()), (2, 2));
    assert_eq!((english.contradictions, german.contradictions), (0, 2));
    Ok(())
}
