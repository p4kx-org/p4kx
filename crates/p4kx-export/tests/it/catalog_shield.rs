// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{DataForge, Value as Raw};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::flow_root_positions::{Step, enumerate};
use crate::health_origin_assertions as proof;

const SOURCE_TYPE: &str = "SCItemShieldGeneratorParams";
const STUN: &str = "FixtureShieldStun";
const SLOT: &str = "FixtureShieldRange";
const OWNER: u128 = 0x7854;
const ROWS: &str = "/configurations/shieldGenerators/rows";
const NUMBERS: [(&str, &str, f32); 10] = [
    ("MaxShieldHealth", "maxHealth", 2048.0),
    ("MaxShieldRegen", "maxRegen", 12.0),
    ("DecayRatio", "decayRatio", 0.25),
    ("DownedRegenDelay", "downedRegenDelay", 4.0),
    ("DamagedRegenDelay", "damagedRegenDelay", 2.0),
    (
        "ReservePoolInitialHealthRatio",
        "reservePoolInitialHealthRatio",
        0.0,
    ),
    (
        "ReservePoolMaxHealthRatio",
        "reservePoolMaxHealthRatio",
        0.5,
    ),
    (
        "ReservePoolRegenRateRatio",
        "reservePoolRegenRateRatio",
        0.75,
    ),
    (
        "ReservePoolDrainRateRatio",
        "reservePoolDrainRateRatio",
        1.25,
    ),
    (
        "ElectricalChargeDamageResistance",
        "electricalChargeDamageResistance",
        -0.0,
    ),
];
const STUN_NUMBERS: [(&str, f32); 4] = [
    ("minAlphaDamageRatio", 0.125),
    ("maxAlphaDamageRatio", 0.875),
    ("minStunTime", 0.0),
    ("maxStunTime", 4.5),
];
const ABSORPTION_VALUES: [(f32, f32); 2] = [(-0.0, 0.125), (0.75, 1.0)];
const SLOT_VALUES: [(f32, f32); 7] = [
    (0.0, 0.125),
    (0.25, 0.375),
    (0.125, 0.75),
    (0.5, 0.625),
    (0.875, 1.0),
    (0.625, 0.75),
    (0.25, 0.5),
];

#[derive(Clone, Copy)]
enum Layout {
    Full,
    Absent,
    Invalid,
}

#[test]
fn shield_generator_keeps_original_values_slots_and_independent_provenance() -> TestResult {
    let source = database(Layout::Full);
    let scalar_ordinals = original_scalar_ordinals(&source)?;
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&source, origin)?;
        let catalog: Value =
            serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
        validator
            .validate(&catalog)
            .map_err(|error| format!("complete catalog schema: {error}"))?;
        let [row] = proof::rows(&catalog, ROWS)? else {
            return Err("only the selected physical shield component is projected".into());
        };
        assert_eq!(
            proof::at(&catalog, "/configurations/shieldGenerators/availability")?,
            "available",
            "the source declares shield generators"
        );
        proof::present(row, "sourceType", &json!(SOURCE_TYPE), Some(encrypted))?;
        proof::present(
            row,
            "ownerGuid",
            &json!(common::guid(OWNER)),
            Some(encrypted),
        )?;
        proof::present(row, "componentOrdinal", &json!(1), Some(encrypted))?;
        verify_values(&catalog, row, &scalar_ordinals, encrypted)?;
        assert!(
            proof::rows(&catalog, "/items")?
                .iter()
                .all(|item| item.pointer("/stats/shield").is_none()),
            "the shield configuration has one canonical wire location"
        );
    }
    Ok(())
}

#[test]
fn shield_distinguishes_undeclared_and_invalid_arrays_from_authored_empty() -> TestResult {
    let (_absent_folder, archive) = common::open(&database(Layout::Absent), DcbOrigin::Encrypted)?;
    let complete: Value =
        serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
    let shield = proof::at(&complete, &format!("{ROWS}/0"))?;
    proof::cell(shield, "shieldResistance", "absent")?;
    proof::present(shield, "shieldAbsorption", &json!([]), Some(true))?;

    let (_invalid_folder, archive) =
        common::open(&database(Layout::Invalid), DcbOrigin::Encrypted)?;
    assert!(
        matches!(
            build_catalog(&archive, &common::options()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "wrong declared storage cannot produce a complete shield snapshot"
    );
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let partial: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    let shield = proof::at(&partial, &format!("{ROWS}/0"))?;
    let failed = proof::cell(shield, "shieldResistance", "invalid")?;
    assert!(
        proof::uses(&partial, failed)?
            .iter()
            .any(|source_use| source_use
                .pointer("/location/field")
                .and_then(Value::as_str)
                == Some("$dataType")),
        "diagnostic uses the original wrong type declaration"
    );
    assert!(
        proof::rows(shield, "/additionalFields/roots")?
            .iter()
            .any(|field| field.pointer("/name/value").and_then(Value::as_str)
                == Some("ShieldResistance")),
        "the unsupported original field remains available"
    );
    Ok(())
}

fn verify_values(catalog: &Value, row: &Value, ordinals: &[u64], encrypted: bool) -> TestResult {
    for ((_, field, value), ordinal) in NUMBERS.into_iter().zip(ordinals) {
        let cell = proof::present(row, field, &number(value), Some(encrypted))?;
        assert!(
            proof::uses(catalog, cell)?
                .iter()
                .any(|source_use| source_use.get("role").and_then(Value::as_str)
                    == Some("payload")
                    && source_use
                        .pointer("/location/namespace")
                        .and_then(Value::as_str)
                        == Some("dataforgeValue")
                    && source_use
                        .pointer("/location/field")
                        .and_then(Value::as_str)
                        == Some("$value")
                    && source_use.pointer("/location/indices")
                        == Some(&json!(["0", ordinal.to_string()]))),
            "{field} points to its actual original scalar"
        );
    }
    let stun = proof::at(proof::cell(row, "stunParams", "present")?, "/value")?;
    proof::present(stun, "sourceType", &json!(STUN), Some(encrypted))?;
    for (field, value) in STUN_NUMBERS {
        proof::present(stun, field, &number(value), Some(encrypted))?;
    }
    verify_ranges(row, "/shieldResistance/value", &SLOT_VALUES, encrypted)?;
    verify_ranges(
        row,
        "/shieldAbsorption/value",
        &ABSORPTION_VALUES,
        encrypted,
    )?;
    let [extra] = proof::rows(row, "/additionalFields/roots")? else {
        return Err("only one unprojected root field".into());
    };
    proof::present(extra, "name", &json!("fixtureEnabled"), Some(encrypted))?;
    let index = proof::at(extra, "/value/valueIndex")?
        .as_str()
        .ok_or("original arena index")?
        .parse::<usize>()?;
    let flag = proof::rows(row, "/additionalFields/values")?
        .get(index)
        .ok_or("original extra field payload")?;
    proof::present(
        flag,
        "value",
        &json!({"type": "boolean", "value": false}),
        Some(encrypted),
    )?;
    Ok(())
}

fn verify_ranges(row: &Value, path: &str, expected: &[(f32, f32)], encrypted: bool) -> TestResult {
    let ranges = proof::rows(row, path)?;
    assert_eq!(
        ranges.len(),
        expected.len(),
        "{path} keeps all original slots"
    );
    for (slot, &(min, max)) in ranges.iter().zip(expected) {
        let slot = proof::at(slot, "/value")?;
        proof::present(slot, "min", &number(min), Some(encrypted))?;
        proof::present(slot, "max", &number(max), Some(encrypted))?;
    }
    Ok(())
}

fn number(value: f32) -> Value {
    json!({"type":"f32", "rawBits":format!("{:08x}", value.to_bits()), "finite":value})
}

fn original_scalar_ordinals(source: &DataForgeBuilder) -> TestResult<Vec<u64>> {
    let bytes = source.build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge.record(0).ok_or("shield fixture owner")?.tree()?;
    let Some(Raw::Array(components)) = tree.field("Components") else {
        return Err("original component storage".into());
    };
    let Some(Raw::Struct(shield)) = components.get(1) else {
        return Err("physical shield fixture slot".into());
    };
    let Some(Raw::Array(resistance)) = shield.field("ShieldResistance") else {
        return Err("original authored resistance array".into());
    };
    assert_eq!(
        resistance.len(),
        SLOT_VALUES.len(),
        "fixture stores all seven slots"
    );
    for (slot, (min, max)) in resistance.iter().zip(SLOT_VALUES) {
        let Raw::Struct(slot) = slot else {
            return Err("original range slot".into());
        };
        assert!(
            matches!(slot.field("Min"), Some(Raw::Single(value)) if value.to_bits() == min.to_bits())
                && matches!(slot.field("Max"), Some(Raw::Single(value)) if value.to_bits() == max.to_bits()),
            "independent original range words"
        );
    }
    let paths = enumerate(tree.root());
    let components = tree
        .fields()
        .iter()
        .position(|field| field.name() == "Components")
        .ok_or("original Components field")?;
    NUMBERS.into_iter().map(|(name, _, expected)| {
        let member = shield.fields().iter().position(|field| field.name() == name)
            .ok_or("original scalar field")?;
        assert!(
            matches!(shield.field(name), Some(Raw::Single(value)) if value.to_bits() == expected.to_bits()),
            "{name} source bits"
        );
        let path = [Step::Field(components), Step::Element(1), Step::Field(member)];
        let ordinal = paths.iter().position(|candidate| *candidate == path)
            .ok_or("original scalar occurrence")?;
        Ok(u64::try_from(ordinal)?)
    }).collect()
}

fn database(layout: Layout) -> DataForgeBuilder {
    let mut shield = TestStruct::new(SOURCE_TYPE)
        .parent("DataForgeComponentParams")
        .field("fixtureEnabled", TestType::Bool)
        .field("stunParams", class(STUN))
        .array("ShieldAbsorption", class(SLOT));
    for (field, _, _) in NUMBERS {
        shield = shield.field(field, TestType::Single);
    }
    shield = match layout {
        Layout::Full => shield.array("ShieldResistance", class(SLOT)),
        Layout::Absent => shield,
        Layout::Invalid => shield.field("ShieldResistance", TestType::Int32),
    };
    let stun = STUN_NUMBERS
        .iter()
        .fold(TestStruct::new(STUN), |schema, (field, _)| {
            schema.field(*field, TestType::Single)
        });
    DataForgeBuilder::new()
        .version(8)
        .enumeration("FixtureShieldItemType", ["Shield", "Misc"])
        .structure(TestStruct::new("EntityClassDefinition").array(
            "Components",
            TestType::StrongPointer("DataForgeComponentParams".to_owned()),
        ))
        .structure(TestStruct::new("DataForgeComponentParams"))
        .structure(
            TestStruct::new("SAttachableComponentParams")
                .parent("DataForgeComponentParams")
                .field("AttachDef", class("SItemDefinition")),
        )
        .structure(
            TestStruct::new("SItemDefinition")
                .field("Type", TestType::Enum("FixtureShieldItemType".to_owned())),
        )
        .structure(shield)
        .structure(stun)
        .structure(
            TestStruct::new(SLOT)
                .field("Min", TestType::Single)
                .field("Max", TestType::Single),
        )
        .record(item(
            OWNER,
            "fixture_shield",
            "Shield",
            vec![shield_value(layout)],
        ))
        .record(item(
            OWNER + 1,
            "fixture_without_shield",
            "Misc",
            Vec::new(),
        ))
}

fn shield_value(layout: Layout) -> TestValue {
    let mut shield = TestInstance::new(SOURCE_TYPE).value("fixtureEnabled", TestValue::Bool(false));
    for (field, _, value) in NUMBERS {
        shield = shield.value(field, TestValue::Single(value));
    }
    let stun = STUN_NUMBERS
        .iter()
        .fold(TestInstance::new(STUN), |stun, (field, value)| {
            stun.value(*field, TestValue::Single(*value))
        });
    let absorption = match layout {
        Layout::Full => ranges(&ABSORPTION_VALUES),
        Layout::Absent | Layout::Invalid => TestValue::Array(Vec::new()),
    };
    shield = shield
        .value("stunParams", TestValue::Struct(stun))
        .value("ShieldAbsorption", absorption);
    TestValue::Struct(match layout {
        Layout::Full => shield.value("ShieldResistance", ranges(&SLOT_VALUES)),
        Layout::Absent => shield,
        Layout::Invalid => shield.value("ShieldResistance", TestValue::Int32(8)),
    })
}

fn ranges(values: &[(f32, f32)]) -> TestValue {
    TestValue::Array(
        values
            .iter()
            .map(|&(min, max)| {
                TestValue::Struct(
                    TestInstance::new(SLOT)
                        .value("Min", TestValue::Single(min))
                        .value("Max", TestValue::Single(max)),
                )
            })
            .collect(),
    )
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

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}
