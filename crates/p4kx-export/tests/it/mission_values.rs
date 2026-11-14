// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::CatalogPublicationMode;
use p4kx_testkit::{TestInstance, TestValue};
use serde_json::{Value, json};

#[path = "mission_values_canonical_auto.rs"]
mod canonical_auto;
#[path = "mission_values_investigation.rs"]
mod investigation;
#[path = "mission_values_schema_boundary.rs"]
mod schema_boundary;

use crate::mission_values_fixture::{self as fixture, Owner, TestResult};

#[test]
fn auto_spawns_retains_every_search_variant_in_all_property_owners() -> TestResult {
    for owner in [Owner::Generator, Owner::Template, Owner::Broker] {
        let document = fixture::build(
            &fixture::database(auto_properties(), owner),
            CatalogPublicationMode::Normal,
        )?;
        verify_auto(&document, owner)?;
    }
    Ok(())
}

fn auto_properties() -> Vec<TestValue> {
    [
        ("Base", TestInstance::new("SpawnClosetSearchOptionsBase")),
        (
            "Entity",
            TestInstance::new("SpawnClosetSearchOptions_EntityDeclaration")
                .value("entityDeclaration", TestValue::Reference(fixture::MISSING)),
        ),
        (
            "Tags",
            TestInstance::new("SpawnClosetSearchOptions_Tags")
                .value("closetPositiveTags", fixture::object(fixture::tags()))
                .value(
                    "closetNegativeTags",
                    fixture::object(TestInstance::new("TagList")),
                ),
        ),
    ]
    .into_iter()
    .map(|(variable, options)| {
        fixture::property(
            variable,
            TestInstance::new("MissionPropertyValue_AutoSpawns")
                .value("autoSpawns", TestValue::Reference(fixture::AUTO_TARGET))
                .value("searchOptions", fixture::object(options)),
        )
    })
    .collect()
}

fn verify_auto(document: &Value, owner: Owner) -> TestResult {
    if !matches!(owner, Owner::Generator) {
        return canonical_auto::variants(document, owner);
    }
    let base = fixture::generator_value(document, "Base")?;
    assert_eq!(
        *fixture::field(base, "/type")?,
        "autoSpawns",
        "the new source union is typed"
    );
    assert_eq!(
        *fixture::field(base, "/searchOptions")?,
        json!({"type":"base"}),
        "schema-only base remains a concrete variant"
    );
    assert_eq!(
        *fixture::field(base, "/autoSpawns/status")?,
        "present",
        "normal References retain resolution"
    );
    let entity = fixture::generator_value(document, "Entity")?;
    assert_eq!(
        *fixture::field(entity, "/searchOptions/entityDeclaration/status")?,
        "dangling",
        "dangling target identity remains represented"
    );
    let tags = fixture::generator_value(document, "Tags")?;
    let positive = tags
        .pointer("/searchOptions/closetPositiveTags/tags")
        .and_then(Value::as_array)
        .ok_or("positive tags missing")?;
    assert_eq!(
        positive.len(),
        4,
        "order, null and repeated References survive"
    );
    assert_eq!(
        positive.first(),
        positive.get(3),
        "duplicates are authored data"
    );
    assert_eq!(
        positive.get(2),
        Some(&Value::Null),
        "null target stays in position"
    );
    assert_eq!(
        tags.pointer("/searchOptions/closetNegativeTags/tags"),
        Some(&json!([])),
        "an explicit empty list remains empty"
    );
    Ok(())
}

#[test]
fn absent_pointer_and_empty_investigation_arrays_are_distinct_source_shapes() -> TestResult {
    let properties = vec![
        fixture::property("Auto", TestInstance::new("MissionPropertyValue_AutoSpawns")),
        fixture::property(
            "Case",
            TestInstance::new("MissionPropertyValue_Investigation"),
        ),
        fixture::property(
            "Race",
            TestInstance::new("MissionPropertyValue_TimeTrialRace"),
        ),
    ];
    let owner = Owner::Template;
    let document = fixture::build(
        &fixture::database(properties, owner),
        CatalogPublicationMode::Normal,
    )?;
    canonical_auto::absent(&document)?;
    Ok(())
}

#[test]
fn auto_spawns_preserves_exact_reference_identities_and_populated_negative_tags() -> TestResult {
    for owner in [Owner::Generator, Owner::Template, Owner::Broker] {
        let negative = TestInstance::new("TagList").value(
            "tags",
            fixture::array(vec![
                TestValue::Reference(fixture::MISSING),
                TestValue::Reference(fixture::TARGET),
            ]),
        );
        let options = TestInstance::new("SpawnClosetSearchOptions_Tags")
            .value("closetPositiveTags", fixture::object(fixture::tags()))
            .value("closetNegativeTags", fixture::object(negative));
        let source = TestInstance::new("MissionPropertyValue_AutoSpawns")
            .value("autoSpawns", TestValue::Reference(fixture::AUTO_TARGET))
            .value("searchOptions", fixture::object(options));
        let document = fixture::build(
            &fixture::database(vec![fixture::property("Raw", source)], owner),
            CatalogPublicationMode::Normal,
        )?;
        if !matches!(owner, Owner::Generator) {
            canonical_auto::identities(&document, owner)?;
            continue;
        }
        let source = fixture::generator_value(&document, "Raw")?;
        let index = 1;
        let auto = fixture::field(source, "/autoSpawns")?;
        for (path, expected) in [
            ("/guid", json!("79000000-0000-4000-8000-000000000007")),
            ("/status", json!("present")),
            ("/targetRecordIndex", json!(2)),
            ("/targetType", json!("AutoSpawns")),
        ] {
            assert_eq!(
                fixture::field(auto, path)?,
                &expected,
                "actual AutoSpawns target {path}"
            );
        }
        let options = fixture::field(source, "/searchOptions")?;
        verify_definition_identity(
            fixture::field(options, "/closetPositiveTags/tags/0")?,
            true,
            index,
        )?;
        verify_definition_identity(
            fixture::field(options, "/closetPositiveTags/tags/1")?,
            false,
            index,
        )?;
        let negative = fixture::field(options, "/closetNegativeTags/tags")?
            .as_array()
            .ok_or("negative tags missing")?;
        assert_eq!(
            negative.len(),
            2,
            "populated negative tag order is independent of the positive list"
        );
        verify_definition_identity(negative.first().ok_or("first negative tag")?, false, index)?;
        verify_definition_identity(negative.get(1).ok_or("second negative tag")?, true, index)?;
    }
    Ok(())
}

fn verify_definition_identity(target: &Value, present: bool, index: u32) -> TestResult {
    let expected = if present {
        "79000000-0000-4000-8000-000000000001"
    } else {
        "79000000-0000-4000-8000-000000000002"
    };
    assert_eq!(
        *fixture::field(target, "/guid")?,
        expected,
        "the original target GUID must not be replaced by a selected target"
    );
    assert_eq!(
        *fixture::field(target, "/status")?,
        if present { "present" } else { "dangling" },
        "source resolution follows that exact identity"
    );
    assert_eq!(
        *fixture::field(target, "/targetRecordIndex")?,
        if present { json!(index) } else { Value::Null },
        "only a physically present source supplies an index"
    );
    if present {
        assert_eq!(
            *fixture::field(target, "/targetType")?,
            "Definition",
            "technical target metadata is retained"
        );
    } else {
        assert_eq!(
            *fixture::field(target, "/recordPath")?,
            Value::Null,
            "a missing target never fabricates a source path"
        );
    }
    Ok(())
}
