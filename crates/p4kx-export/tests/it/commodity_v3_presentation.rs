// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use crate::commodity_v3_fixture::{self as fixture, TestResult};
use crate::commodity_v3_source;
use crate::commodity_v3_source_inventory::{body, header};

#[test]
fn untranslated_own_names_win_before_lookup_and_unused_resource_inputs_do_not_leak() -> TestResult {
    let (_, catalog) = fixture::build(&fixture::database())?;
    let row = fixture::presentation(&catalog, fixture::ALPHA)?;
    let chosen = fixture::text_row(&catalog, "publicNames", &row["name"]["value"])?;
    assert_eq!(chosen["key"]["value"], "@own_untranslated");
    for language in ["en", "de"] {
        assert_eq!(
            chosen["translations"]["value"][language]["state"], "absent",
            "an available translated resource key is not a language fallback"
        );
    }
    let expected = BTreeSet::from([
        (
            fixture::ALPHA_INDEX,
            body("Components"),
            "selector".to_owned(),
        ),
        (
            fixture::ALPHA_INDEX,
            body("Components[0]"),
            "selector".to_owned(),
        ),
        (
            fixture::ALPHA_INDEX,
            body("Components[1].$type"),
            "selector".to_owned(),
        ),
        (
            fixture::ALPHA_INDEX,
            body("Components[2].$type"),
            "selector".to_owned(),
        ),
        (
            fixture::ALPHA_INDEX,
            body("Components[2].displayName"),
            "selector".to_owned(),
        ),
        (
            fixture::ALPHA_INDEX,
            body("Components[2].displayName"),
            "payload".to_owned(),
        ),
        (
            fixture::ALPHA_INDEX,
            body("Components[2].displayName"),
            "conversion".to_owned(),
        ),
    ]);
    assert_eq!(
        commodity_v3_source::uses(&catalog, &row["name"])?,
        expected,
        "the winning own key has its exact physical selectors and value inputs"
    );
    assert_eq!(
        row["name"]["containsEncryptedSource"], false,
        "loaded encrypted locale payload is not part of the chosen key reference"
    );
    Ok(())
}

#[test]
fn placeholder_fallback_uses_the_actual_resource_owner_and_only_its_retained_key_payload()
-> TestResult {
    let (_, catalog) = fixture::build(&fixture::database())?;
    let row = fixture::presentation(&catalog, fixture::BETA)?;
    let chosen = fixture::text_row(&catalog, "publicNames", &row["name"]["value"])?;
    assert_eq!(chosen["key"]["value"], "@resource_name");
    assert_eq!(
        chosen["translations"]["value"]["en"]["value"],
        "Resource English"
    );
    assert_eq!(
        chosen["translations"]["value"]["en"]["containsEncryptedSource"],
        true
    );
    assert_eq!(
        chosen["translations"]["value"]["de"]["containsEncryptedSource"],
        false
    );
    let uses = commodity_v3_source::uses(&catalog, &row["name"])?;
    let actual: BTreeSet<_> = uses
        .iter()
        .filter(|(_, _, role)| role == "payload" || role == "conversion")
        .cloned()
        .collect();
    assert_eq!(
        actual,
        BTreeSet::from([
            (0, body("displayName"), "payload".to_owned()),
            (0, body("displayName"), "conversion".to_owned()),
        ]),
        "the resource key is not misattributed to the Entity or its placeholder"
    );
    assert!(uses.contains(&(
        fixture::BETA_INDEX,
        body("Components[0].displayName"),
        "selector".to_owned()
    )));
    assert!(uses.contains(&(
        fixture::BETA_INDEX,
        body("Components[1].defaultComposition[0].entry"),
        "selector".to_owned()
    )));
    assert!(
        uses.iter()
            .all(|(record, _, _)| *record == 0 || *record == fixture::BETA_INDEX),
        "unrelated loaded resource and group rows cannot taint name selection"
    );
    Ok(())
}

#[test]
fn placeholder_precedence_is_preserved_even_when_the_fallback_is_also_a_placeholder() -> TestResult
{
    let (_, catalog) = fixture::build(&fixture::with_keys(
        "@LOC_EMPTY",
        "@own_description",
        "@blank_space",
    ))?;
    let row = fixture::presentation(&catalog, fixture::ALPHA)?;
    let chosen = fixture::text_row(&catalog, "publicNames", &row["name"]["value"])?;
    assert_eq!(
        chosen["key"]["value"], "@blank_space",
        "a nonempty resource key precedes the original own placeholder"
    );
    for language in ["en", "de"] {
        assert_eq!(chosen["translations"]["value"][language]["state"], "absent");
        assert_eq!(
            chosen["translations"]["value"][language]["containsEncryptedSource"], false,
            "an unused encrypted locale is not a contributor"
        );
    }
    Ok(())
}

#[test]
fn first_group_choice_uses_source_record_order_while_density_and_refining_remain_resource_facts()
-> TestResult {
    let (_, catalog) = fixture::build(&fixture::database())?;
    let row = fixture::presentation(&catalog, fixture::ALPHA)?;
    const {
        assert!(
            fixture::GROUP_SECOND < fixture::GROUP_FIRST,
            "the fixture distinguishes scan order from GUID sort"
        );
    };
    assert_eq!(
        row["typeGuid"]["value"],
        fixture::guid(fixture::GROUP_FIRST)
    );
    assert_eq!(row["densityGramsPerCubicCentimetre"]["value"], 3.25);
    assert_eq!(
        row["refinedSubtypeGuid"]["value"],
        fixture::guid(fixture::RESOURCE_FALLBACK)
    );
    let uses = commodity_v3_source::uses(&catalog, &row["typeGuid"])?;
    assert!(
        uses.contains(&(3, header("guid"), "payload".to_owned())),
        "the selected group owns the value"
    );
    assert!(
        !uses.contains(&(4, header("guid"), "payload".to_owned())),
        "a later lower GUID is not selected"
    );
    let fallback = fixture::presentation(&catalog, fixture::RESOURCE_FALLBACK)?;
    assert!(
        fallback["densityGramsPerCubicCentimetre"]["value"].is_null(),
        "no density is manufactured"
    );
    assert!(
        fallback["refinedSubtypeGuid"]["value"].is_null(),
        "a missing refined link stays no chosen target"
    );
    Ok(())
}
