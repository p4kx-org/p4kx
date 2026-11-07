// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::CatalogPublicationMode;
use p4kx_testkit::{TestInstance, TestValue};
use serde_json::{Value, json};

#[path = "mission_values_investigation.rs"]
mod investigation;

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
                .value("autoSpawns", TestValue::Reference(fixture::TARGET))
                .value("searchOptions", fixture::object(options)),
        )
    })
    .collect()
}

fn verify_auto(document: &Value, owner: Owner) -> TestResult {
    let base = fixture::value(document, owner, "Base")?;
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
    let entity = fixture::value(document, owner, "Entity")?;
    assert_eq!(
        *fixture::field(entity, "/searchOptions/entityDeclaration/status")?,
        "dangling",
        "dangling target identity remains represented"
    );
    let tags = fixture::value(document, owner, "Tags")?;
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
    let auto = fixture::value(&document, owner, "Auto")?;
    assert_eq!(
        *fixture::field(auto, "/autoSpawns")?,
        Value::Null,
        "absent normal Reference is null"
    );
    assert_eq!(
        *fixture::field(auto, "/searchOptions")?,
        Value::Null,
        "absent optional pointer is null"
    );
    assert_eq!(
        *fixture::field(fixture::value(&document, owner, "Case")?, "/suspects")?,
        json!([]),
        "declared empty Class arrays remain empty"
    );
    assert_eq!(
        *fixture::field(fixture::value(&document, owner, "Race")?, "/targetSplits")?,
        json!([]),
        "declared empty split arrays remain empty"
    );
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
            .value("autoSpawns", TestValue::Reference(fixture::TARGET))
            .value("searchOptions", fixture::object(options));
        let document = fixture::build(
            &fixture::database(vec![fixture::property("Raw", source)], owner),
            CatalogPublicationMode::Normal,
        )?;
        let source = fixture::value(&document, owner, "Raw")?;
        let index = match owner {
            Owner::Generator => 1,
            Owner::Template | Owner::Broker => 2,
        };
        verify_definition_identity(fixture::field(source, "/autoSpawns")?, true, index)?;
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
