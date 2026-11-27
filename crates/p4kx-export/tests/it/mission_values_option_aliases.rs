// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::CatalogPublicationMode;
use p4kx_testkit::{DataForgeBuilder, TestValue};
use serde_json::json;

use super::{declaration, option, verify_generator_display};
use crate::health_origin_assertions as proof;
use crate::mission_values_fixture::{self as fixture, Owner, TestResult};

#[test]
fn identical_option_aliases_publish_one_sourced_name_for_each_authored_set() -> TestResult {
    let label = "@option_alias";
    let document = fixture::build(&option_aliases([label; 2]), CatalogPublicationMode::Normal)?;
    let sets = proof::rows(&document, "/missionPropertySets")?;
    assert_eq!(
        sets.len(),
        4,
        "the same authored sets coalesce across physical aliases"
    );
    let mut selected_owner = None;
    for set in sets {
        let option = proof::at(set, "/properties/0/value/options/0")?;
        verify_generator_display(&document, option, label)?;
        let display = proof::cell(option, "displayName", "present")?;
        for used in proof::uses(&document, display)? {
            if used.pointer("/location/namespace") != Some(&json!("dataforgeValue")) {
                continue;
            }
            let owner = proof::at(used, "/location/indices/0")?
                .as_str()
                .ok_or("the physical Generator record index")?;
            if let Some(previous) = selected_owner {
                assert_eq!(
                    owner, previous,
                    "equivalent aliases retain the selected source owner"
                );
            }
            selected_owner = Some(owner);
        }
    }
    assert!(
        selected_owner.is_some(),
        "the selected owner has a physical coordinate"
    );
    Ok(())
}

#[test]
fn differing_option_aliases_quarantine_their_sets_before_publishing_text() -> TestResult {
    let labels = ["@option_alias_a", "@option_alias_b"];
    let document = fixture::build(
        &option_aliases(labels),
        CatalogPublicationMode::DiagnosticPartial,
    )?;
    assert!(
        proof::rows(&document, "/missionPropertySets")?.is_empty(),
        "the option key is part of the authored payload used to compare aliases",
    );
    for name in ["contracts", "missionPropertySets"] {
        let health = proof::rows(&document, "/exportHealth/kinds")?
            .iter()
            .find(|kind| kind["kind"] == name)
            .ok_or("kind health")?;
        assert_eq!(
            health["failedDerivedProjections"], 8,
            "each of four conflicting sets is counted for both physical owners",
        );
    }
    for row in proof::rows(&document, "/texts/gameTexts")? {
        let key = proof::at(row, "/key/value")?;
        assert!(
            !labels.iter().any(|label| key == *label),
            "a quarantined option cannot register an orphan text row",
        );
    }
    assert!(
        proof::rows(&document, "/exportHealth/diagnostics")?
            .iter()
            .any(|row| row["warningCode"] == "ambiguous_mission_property_set"),
        "the differing original option keys retain the canonical merge diagnostic",
    );
    Ok(())
}

fn option_aliases([left, right]: [&str; 2]) -> DataForgeBuilder {
    use crate::mission_property_sets_fixture::{generator, parameters};

    let label = |key: &str| {
        declaration(
            "Integer",
            vec![fixture::object(
                option("Integer").value("textId", TestValue::text(key)),
            )],
        )
    };
    fixture::database(vec![label(left)], Owner::Generator).record(generator(std::array::from_fn(
        |_| parameters(vec![label(right)]),
    )))
}
