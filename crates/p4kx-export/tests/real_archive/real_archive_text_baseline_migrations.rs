// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use super::{Fixture, TestResult};
use crate::real_archive_texts::check_coverage;

const GAMEPLAY: &str = "CraftingGameplayPropertyDef";
const OVERRIDE: &str = "CraftingPropertyNameOverride";
const CONDITION: &str = "CraftingPropertyNameOverrideCondition_Base";

#[test]
fn migrated_baselines_include_original_names_and_check_new_authored_texts() -> TestResult {
    let fixture = Fixture::from_database(&database())?;
    for (kind, historic_field, supplemental_field, domain) in [
        (
            Kind::ResourceGroups,
            "name",
            "/description/value",
            "gameTexts",
        ),
        (
            Kind::GameplayProperties,
            "propertyName",
            "/unitFormat/value",
            "publicNames",
        ),
        (
            Kind::ReputationScopes,
            "displayName",
            "/description/value",
            "gameTexts",
        ),
        (
            Kind::ReputationStandings,
            "displayName",
            "/perkDescription/value",
            "gameTexts",
        ),
    ] {
        check_baseline(&fixture, kind, historic_field)?;
        check_mutation(&fixture, kind, supplemental_field, domain)?;
    }
    check_mutation(
        &fixture,
        Kind::GameplayProperties,
        "/nameOverrides/value/0/value/payload/value/propertyName/value",
        "publicNames",
    )
}

fn check_baseline(fixture: &Fixture, kind: Kind, name: &str) -> TestResult {
    let row = format!("/definitions/{}/rows/0", kind.name());
    let mut catalog = fixture.catalog.clone();
    let (english, german) = fixture.coverage(&catalog, kind)?;
    assert_eq!(
        english.total(),
        1,
        "only the original name enters the English baseline"
    );
    assert_eq!(
        german.total(),
        1,
        "only the original name enters the German baseline"
    );
    assert_eq!(
        german.resolved, 1,
        "supplemental gaps do not dilute the baseline"
    );
    replace(
        &mut catalog,
        &format!("{row}/{name}/state"),
        json!("absent"),
    )?;
    replace(&mut catalog, &format!("{row}/{name}/value"), Value::Null)?;
    let (english, german) = fixture.coverage(&catalog, kind)?;
    let mut failures = Vec::new();
    check_coverage(kind, &english, &german, true, 1, &mut failures);
    assert_eq!(
        failures.len(),
        1,
        "an admitted row cannot evade its required baseline"
    );
    Ok(())
}

fn check_mutation(fixture: &Fixture, kind: Kind, field: &str, domain: &str) -> TestResult {
    let mut catalog = fixture.catalog.clone();
    let row = format!("/definitions/{}/rows/0{field}", kind.name());
    let target = catalog
        .pointer(&row)
        .and_then(Value::as_str)
        .ok_or("the producer retains this supplemental reference")?
        .to_owned();
    let registry = catalog
        .pointer_mut(&format!("/texts/{domain}"))
        .and_then(Value::as_array_mut)
        .ok_or("canonical text domain")?;
    let text = registry
        .iter_mut()
        .find(|row| row.pointer("/id/value").and_then(Value::as_str) == Some(&target))
        .ok_or("original text reference target")?;
    replace(
        text,
        "/translations/value/en/displayText/value",
        json!("Wrong English text"),
    )?;
    let (english, german) = fixture.coverage(&catalog, kind)?;
    assert_eq!(
        english.total(),
        1,
        "changing a supplemental field does not change the English denominator"
    );
    assert_eq!(
        english.supplemental_contradictions, 1,
        "an added field still compares to its true source"
    );
    assert_eq!(
        german.supplemental_contradictions, 0,
        "the untouched localised form still agrees"
    );
    let mut failures = Vec::new();
    check_coverage(kind, &english, &german, true, 1, &mut failures);
    assert_eq!(
        failures.len(),
        1,
        "a supplemental source contradiction independently fails"
    );
    Ok(())
}

fn replace(value: &mut Value, path: &str, replacement: Value) -> TestResult {
    *value
        .pointer_mut(path)
        .ok_or("required test mutation field")? = replacement;
    Ok(())
}

fn database() -> DataForgeBuilder {
    let [group, scope, standing] = named_records();
    DataForgeBuilder::new()
        .version(8)
        .structure(named_type(
            "ResourceTypeGroup",
            "displayName",
            "description",
        ))
        .structure(named_type(
            "SReputationScopeParams",
            "displayName",
            "description",
        ))
        .structure(named_type(
            "SReputationStandingParams",
            "displayName",
            "perkDescription",
        ))
        .structure(
            named_type(GAMEPLAY, "propertyName", "unitFormat")
                .array("nameOverrides", TestType::Class(OVERRIDE.to_owned())),
        )
        .structure(
            TestStruct::new(OVERRIDE)
                .field("propertyName", TestType::Locale)
                .field("condition", TestType::StrongPointer(CONDITION.to_owned())),
        )
        .structure(TestStruct::new(CONDITION))
        .record(group)
        .record(
            named_record(
                2,
                GAMEPLAY,
                "crafting/craftedproperties",
                "propertyName",
                "unitFormat",
            )
            .value(
                "nameOverrides",
                TestValue::Array(vec![TestValue::Struct(
                    TestInstance::new(OVERRIDE)
                        .value("propertyName", TestValue::text("@later_name")),
                )]),
            ),
        )
        .record(scope)
        .record(standing)
}

fn named_records() -> [TestRecord; 3] {
    let group = named_record(
        1,
        "ResourceTypeGroup",
        "resourcetypedatabase",
        "displayName",
        "description",
    );
    let scope = named_record(
        3,
        "SReputationScopeParams",
        "reputation/scopes",
        "displayName",
        "description",
    );
    let standing = named_record(
        4,
        "SReputationStandingParams",
        "reputation/standings",
        "displayName",
        "perkDescription",
    );
    [group, scope, standing]
}

fn named_type(kind: &str, first: &str, second: &str) -> TestStruct {
    TestStruct::new(kind)
        .field(first, TestType::Locale)
        .field(second, TestType::Locale)
}

fn named_record(guid: u128, kind: &str, folder: &str, first: &str, second: &str) -> TestRecord {
    TestRecord::new(
        guid,
        kind,
        format!("{kind}.Fixture"),
        format!("libs/foundry/records/{folder}/Fixture.xml"),
    )
    .value(first, TestValue::text("@shared"))
    .value(second, TestValue::text("@own_type"))
}
