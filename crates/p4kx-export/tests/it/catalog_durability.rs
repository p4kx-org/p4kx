// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "catalog_durability_death_explosion.rs"]
mod death_explosion;
#[path = "catalog_durability_event_actions.rs"]
mod event_actions;
#[path = "catalog_durability_lifecycle.rs"]
mod lifecycle;
#[path = "catalog_durability_nested.rs"]
mod nested;
#[path = "catalog_durability_number_controls.rs"]
mod number_controls;
#[path = "catalog_durability_resistance_reference.rs"]
mod resistance_reference;

use p4kx_export::{CatalogError, CatalogPublicationMode, build_catalog};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

const HEALTH_ONLY: u128 = 0x7001;
const PLAIN: u128 = 0x7002;
const SPECIALIST: u128 = 0x7003;
const HEALTH: &str = "SHealthComponentParams";
const DURABILITY: &str = "/configurations/durability";

#[test]
fn durability_keeps_health_only_and_every_original_component_occurrence() -> TestResult {
    let source = good_database();
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&source, origin)?;
        let built = build_catalog(&archive, &common::options())?;
        let catalog = serde_json::from_slice(built.json())?;
        assert_eq!(
            proof::at(&catalog, "/exportHealth/status")?,
            "complete",
            "valid exact values"
        );
        assert_eq!(
            proof::at(&catalog, &format!("{DURABILITY}/availability"))?,
            "available",
            "the actual Health source family exists"
        );
        let rows = proof::rows(&catalog, &format!("{DURABILITY}/rows"))?;
        let [first, repeated, shield] = rows else {
            return Err("two health-only occurrences and one specialist occurrence".into());
        };
        assert_health_only(&catalog, first, 1, "00000000", Some(0.0), encrypted)?;
        assert_health_only(&catalog, repeated, 3, "7fc01234", None, encrypted)?;
        proof::present(
            shield,
            "ownerGuid",
            &json!(common::guid(SPECIALIST)),
            Some(encrypted),
        )?;
        proof::present(shield, "componentOrdinal", &json!(2), Some(encrypted))?;
        proof::present(
            shield,
            "health",
            &json!({"rawBits": "43e10000", "finite": 450.0}),
            Some(encrypted),
        )?;
        assert_legacy_gate(&catalog)?;
    }
    Ok(())
}

#[test]
fn durability_diagnoses_wrong_width_and_preserves_unreadable_health_separately() -> TestResult {
    let source = database(Some(TestType::String)).record(item(
        HEALTH_ONLY,
        "durability_bad",
        "Misc",
        vec![
            health(TestValue::text("wrong type")),
            health(TestValue::Raw(u32::MAX.to_le_bytes().to_vec())),
        ],
    ));
    let (_folder, archive) = common::open(&source, DcbOrigin::Plain)?;
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    let catalog = serde_json::from_slice(built.json())?;
    let rows = proof::rows(&catalog, &format!("{DURABILITY}/rows"))?;
    let [wrong_type, unreadable] = rows else {
        return Err("both actual Health components must remain visible".into());
    };
    let invalid = proof::cell(wrong_type, "health", "invalid")?;
    assert_eq!(
        invalid.get("sourceSetComplete"),
        Some(&json!(true)),
        "readable wrong storage"
    );
    assert_site(&catalog, invalid, "5", "$value", "selector")?;
    let unreadable = proof::cell(unreadable, "health", "unreadable")?;
    assert_eq!(
        unreadable.get("sourceSetComplete"),
        Some(&json!(false)),
        "a failed read is incomplete"
    );
    assert!(
        proof::uses(&catalog, unreadable)?
            .iter()
            .all(|usage| usage.get("role").and_then(Value::as_str) != Some("attempted")),
        "the failed read does not contribute a value"
    );
    assert_receipt(
        proof::rows(&catalog, "/provenance/uses")?.iter(),
        "7",
        "$value",
        "attempted",
    );
    assert_eq!(
        proof::at(&catalog, "/exportHealth/status")?,
        "incomplete",
        "actual failed projection"
    );
    assert!(
        matches!(
            build_catalog(&archive, &common::options()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "normal export refuses malformed Health"
    );
    Ok(())
}

#[test]
fn durability_distinguishes_missing_health_from_no_component() -> TestResult {
    let source = database(None)
        .record(item(
            HEALTH_ONLY,
            "durability_missing",
            "Misc",
            vec![component(HEALTH)],
        ))
        .record(item(PLAIN, "durability_plain", "Misc", Vec::new()));
    let (_, catalog) = common::build(&source)?;
    let [only] = proof::rows(&catalog, &format!("{DURABILITY}/rows"))? else {
        return Err("only the actual SHealth component supplies a durability row".into());
    };
    let absent = proof::cell(only, "health", "absent")?;
    assert_eq!(
        absent.get("sourceSetComplete"),
        Some(&json!(true)),
        "complete declaration survey"
    );
    assert_site(&catalog, absent, "4", "$fields", "selector")?;
    Ok(())
}

fn assert_health_only(
    catalog: &Value,
    row: &Value,
    slot: u64,
    bits: &str,
    finite: Option<f32>,
    encrypted: bool,
) -> TestResult {
    proof::present(
        row,
        "ownerGuid",
        &json!(common::guid(HEALTH_ONLY)),
        Some(encrypted),
    )?;
    proof::present(row, "ownerRecordIndex", &json!(0), Some(encrypted))?;
    let ordinal = proof::present(row, "componentOrdinal", &json!(slot), Some(encrypted))?;
    let health = proof::present(
        row,
        "health",
        &json!({"rawBits": bits, "finite": finite}),
        Some(encrypted),
    )?;
    let (component, field) = if slot == 1 { ("4", "5") } else { ("7", "8") };
    assert_site(catalog, ordinal, component, "$value", "conversion")?;
    assert_site(catalog, health, component, "$type", "selector")?;
    assert_site(catalog, health, field, "$value", "payload")?;
    Ok(())
}

fn assert_site(
    catalog: &Value,
    cell: &Value,
    ordinal: &str,
    field: &str,
    role: &str,
) -> TestResult {
    assert_receipt(proof::uses(catalog, cell)?, ordinal, field, role);
    Ok(())
}

fn assert_receipt<'source>(
    observed: impl IntoIterator<Item = &'source Value>,
    ordinal: &str,
    field: &str,
    role: &str,
) {
    assert!(
        observed.into_iter().any(|usage| {
            usage.get("role").and_then(Value::as_str) == Some(role)
                && usage.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeValue")
                && usage.pointer("/location/indices") == Some(&json!(["0", ordinal]))
                && usage.pointer("/location/field").and_then(Value::as_str) == Some(field)
        }),
        "original Record 0 value {ordinal} field {field} role {role}"
    );
}

fn assert_legacy_gate(catalog: &Value) -> TestResult {
    let items = proof::rows(catalog, "/items")?;
    let own = common::guid(HEALTH_ONLY);
    let specialist = common::guid(SPECIALIST);
    let lookup = |guid: &str| {
        items
            .iter()
            .find(|row| row.get("guid").and_then(Value::as_str) == Some(guid))
    };
    assert_eq!(
        lookup(&own).and_then(|row| row.get("stats")),
        Some(&Value::Null),
        "temporary legacy presence semantics"
    );
    assert_eq!(
        lookup(&specialist).and_then(|row| row.pointer("/stats/health")),
        Some(&json!(450.0)),
        "temporary legacy specialist value stays exact"
    );
    Ok(())
}

fn good_database() -> DataForgeBuilder {
    database(Some(TestType::Single))
        .record(item(
            HEALTH_ONLY,
            "durability_only",
            "Misc",
            vec![
                health(TestValue::Single(0.0)),
                component("FixtureOther"),
                health(TestValue::Single(f32::from_bits(0x7fc0_1234))),
            ],
        ))
        .record(item(PLAIN, "durability_plain", "Misc", Vec::new()))
        .record(item(
            SPECIALIST,
            "durability_specialist",
            "Shield",
            vec![
                TestValue::Struct(
                    TestInstance::new("SCItemShieldGeneratorParams")
                        .value("MaxShieldHealth", TestValue::Single(8_000.0)),
                ),
                health(TestValue::Single(450.0)),
            ],
        ))
}

fn database(health_type: Option<TestType>) -> DataForgeBuilder {
    let health = TestStruct::new(HEALTH).parent("DataForgeComponentParams");
    let health = match health_type {
        Some(health_type) => health.field("Health", health_type),
        None => health,
    };
    database_with_health(health)
}

fn database_with_health(health: TestStruct) -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .enumeration("FixtureItemType", ["Misc", "Shield"])
        .structure(TestStruct::new("EntityClassDefinition").array(
            "Components",
            TestType::StrongPointer("DataForgeComponentParams".to_owned()),
        ))
        .structure(TestStruct::new("DataForgeComponentParams"))
        .structure(
            TestStruct::new("SAttachableComponentParams")
                .parent("DataForgeComponentParams")
                .field("AttachDef", TestType::Class("SItemDefinition".to_owned())),
        )
        .structure(
            TestStruct::new("SItemDefinition")
                .field("Type", TestType::Enum("FixtureItemType".to_owned())),
        )
        .structure(TestStruct::new("FixtureOther").parent("DataForgeComponentParams"))
        .structure(
            TestStruct::new("SCItemShieldGeneratorParams")
                .parent("DataForgeComponentParams")
                .field("MaxShieldHealth", TestType::Single),
        )
        .structure(health)
}

fn item(guid: u128, name: &str, kind: &str, extra: Vec<TestValue>) -> TestRecord {
    let definition = TestInstance::new("SItemDefinition").value("Type", TestValue::text(kind));
    let attach = TestInstance::new("SAttachableComponentParams")
        .value("AttachDef", TestValue::Struct(definition));
    let mut components = vec![TestValue::Struct(attach)];
    components.extend(extra);
    TestRecord::new(
        guid,
        "EntityClassDefinition",
        format!("EntityClassDefinition.{name}"),
        format!("libs/foundry/records/entities/scitem/ships/{name}.xml"),
    )
    .value("Components", TestValue::Array(components))
}

fn component(name: &str) -> TestValue {
    TestValue::Struct(TestInstance::new(name))
}

fn health(value: TestValue) -> TestValue {
    TestValue::Struct(TestInstance::new(HEALTH).value("Health", value))
}
