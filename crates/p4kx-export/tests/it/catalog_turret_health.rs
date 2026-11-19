// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog, catalog_schema};
use p4kx_extraction::Filter;
use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestInstance, TestRecord, TestStruct, TestType, TestValue,
};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_turret_pointer_probe.rs"]
mod pointer_probe;

#[path = "catalog_turret_health_sdk_scope.rs"]
mod sdk_scope;

const TURRET: &str = "SCItemTurretParams";
const MODIFIER: &str = "STurretHealthModifierDef";
const CURVE: &str = "BezierCurve";
const DERIVED: &str = "FixtureDerivedHealthModifier";
const DERIVED_OWNER: u128 = 0x7314;
const DERIVED_TARGET: u128 = 0x7315;
const DERIVED_OWNER_PATH: &str = "libs/foundry/records/entities/fixtures/turret_derived.xml";
const POINT: &str = "Vec2";
const BASE: &str = "DataForgeComponentParams";
const OWNER: u128 = 0x7310;
const DEFINITION: u128 = 0x7311;
const STANDALONE: u128 = 0x7312;
const MISSING: u128 = 0x7313;
const OWNER_PATH: &str = "libs/foundry/records/entities/fixtures/turret.xml";
const DEFINITION_PATH: &str = "libs/foundry/records/fixtures/modifier_linked.xml";
const DUPLICATE_PATH: &str = "libs/foundry/records/fixtures/modifier_duplicate.xml";
const STANDALONE_PATH: &str = "libs/foundry/records/fixtures/modifier_standalone.xml";
const COMPONENT_ROWS: &str = "/configurations/turretHealth/rows";
const DEFINITION_ROWS: &str = "/definitions/turretHealthModifiers/rows";

#[test]
fn turret_health_exports_physical_components_and_their_selected_definitions() -> TestResult {
    let fixture = database(true);
    let validator = jsonschema::validator_for(&catalog_schema())?;
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let filter = Filter::parse(OWNER_PATH.trim_end_matches(".xml"))?;
        let catalog = export(&fixture, origin, Some(&filter))?;
        validator
            .validate(&catalog)
            .map_err(|error| format!("turret health schema: {error}"))?;
        let [linked, null, dangling] = proof::rows(&catalog, COMPONENT_ROWS)? else {
            return Err("every physical component in the selected owning Record".into());
        };
        for (ordinal, row) in [linked, null, dangling].into_iter().enumerate() {
            proof::present(
                row,
                "ownerGuid",
                &json!(common::guid(OWNER)),
                Some(encrypted),
            )?;
            proof::present(row, "ownerRecordIndex", &json!(0), Some(encrypted))?;
            proof::present(row, "componentOrdinal", &json!(ordinal), Some(encrypted))?;
        }

        let link = proof::at(linked, "/healthModifierRecord/value")?;
        assert_eq!(link["status"], "present", "the matching original Reference");
        let [definition] = proof::rows(&catalog, DEFINITION_ROWS)? else {
            return Err("one original physical dependency in the filtered catalog".into());
        };
        assert_eq!(
            proof::at(link, "/targetRecordIndex/value")?,
            proof::at(definition, "/source/recordIndex/value")?,
            "the Reference selects the exact physical modifier"
        );
        proof::present(
            link,
            "guid",
            &json!(common::guid(DEFINITION)),
            Some(encrypted),
        )?;
        proof::present(link, "targetRecordIndex", &json!(1), Some(encrypted))?;
        proof::present(link, "targetType", &json!(MODIFIER), Some(encrypted))?;
        check_unlinked(null, dangling, encrypted)?;
        additional_member(linked, "fixtureTurret", encrypted)?;
        check_definition(definition, encrypted)?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn turret_health_keeps_standalone_and_duplicate_modifier_records() -> TestResult {
    let fixture = database(true);
    let catalog = export(&fixture, DcbOrigin::Plain, None)?;
    let [linked, duplicate, standalone] = proof::rows(&catalog, DEFINITION_ROWS)? else {
        return Err("three separate physical Records".into());
    };
    for (index, definition) in [(1, linked), (2, duplicate), (3, standalone)] {
        let source = proof::at(definition, "/source")?;
        proof::present(source, "recordIndex", &json!(index), Some(false))?;
        proof::present(source, "sourceType", &json!(MODIFIER), Some(false))?;
    }
    assert_eq!(
        proof::at(linked, "/source/guid/value")?,
        proof::at(duplicate, "/source/guid/value")?,
        "a shared GUID does not merge two physical definitions"
    );
    let filter = Filter::parse(STANDALONE_PATH.trim_end_matches(".xml"))?;
    let scoped = export(&fixture, DcbOrigin::Plain, Some(&filter))?;
    assert_eq!(
        proof::rows(&scoped, COMPONENT_ROWS)?,
        &[] as &[Value],
        "a definition-only filter yields no turret owners"
    );
    let [row] = proof::rows(&scoped, DEFINITION_ROWS)? else {
        return Err("independently selected definition".into());
    };
    assert_eq!(
        proof::at(row, "/source/recordIndex/value")?,
        3,
        "the standalone original Record"
    );
    check_definition(row, false)?;
    Ok(())
}

#[test]
fn turret_health_requires_the_exact_scalar_reference_declaration() -> TestResult {
    let fixture = database(false);
    let filter = Filter::parse(OWNER_PATH.trim_end_matches(".xml"))?;
    let catalog = export(&fixture, DcbOrigin::Encrypted, Some(&filter))?;
    let [row, ..] = proof::rows(&catalog, COMPONENT_ROWS)? else {
        return Err("malformed turret declaration is still identifiable".into());
    };
    proof::cell(row, "healthModifierRecord", "unsupported")?;
    additional_member(row, "healthModifierRecord", true)?;
    assert_eq!(
        proof::rows(&catalog, DEFINITION_ROWS)?,
        &[] as &[Value],
        "a rejected Reference adds no definition dependency"
    );
    proof::closure(&catalog)
}

#[test]
fn turret_health_keeps_a_compatible_derived_reference_target() -> TestResult {
    let fixture = derived_database();
    let filter = Filter::parse(DERIVED_OWNER_PATH.trim_end_matches(".xml"))?;
    let catalog = export(&fixture, DcbOrigin::Plain, Some(&filter))?;
    let [component] = proof::rows(&catalog, COMPONENT_ROWS)? else {
        return Err("selected original turret component".into());
    };
    let [definition] = proof::rows(&catalog, DEFINITION_ROWS)? else {
        return Err("compatible derived dependency".into());
    };
    let link = proof::at(component, "/healthModifierRecord/value")?;
    assert_eq!(
        link["status"], "present",
        "the resolved target derives from the declared definition"
    );
    proof::present(link, "targetType", &json!(DERIVED), Some(false))?;
    assert_eq!(
        proof::at(link, "/targetRecordIndex/value")?,
        proof::at(definition, "/source/recordIndex/value")?,
        "the compatible derived Record is the target of the Reference",
    );
    check_definition(definition, false)?;
    let derived_index = proof::at(definition, "/source/recordIndex/value")?
        .as_u64()
        .ok_or("physical derived definition Record")?;
    sdk_scope::check_derived(&fixture, u32::try_from(derived_index)?)?;
    proof::closure(&catalog)
}

fn check_unlinked(null: &Value, dangling: &Value, encrypted: bool) -> TestResult {
    let missing = proof::at(dangling, "/healthModifierRecord/value")?;
    assert_eq!(
        missing["status"], "dangling",
        "the stored missing Reference"
    );
    proof::present(
        missing,
        "guid",
        &json!(common::guid(MISSING)),
        Some(encrypted),
    )?;
    proof::cell(missing, "targetRecordIndex", "absent")?;
    let null = proof::cell(null, "healthModifierRecord", "null")?;
    assert_eq!(
        null["containsEncryptedSource"],
        json!(encrypted),
        "the null Reference has its original marker"
    );
    Ok(())
}

fn check_definition(definition: &Value, encrypted: bool) -> TestResult {
    additional_member(definition, "fixtureDefinition", encrypted)?;
    let curve = proof::at(definition, "/damageMovementModifier/value")?;
    proof::present(curve, "sourceType", &json!(CURVE), Some(encrypted))?;
    proof::present(curve, "useLUT", &json!(true), Some(encrypted))?;
    additional_member(curve, "fixtureCurve", encrypted)?;
    let [first, second] = proof::rows(curve, "/points/value")? else {
        return Err("two authored positions in original order".into());
    };
    for (slot, (x, y)) in [first, second]
        .into_iter()
        .zip([(0.0_f32, 4.5_f32), (-1.0, -0.0)])
    {
        assert_eq!(
            slot["containsEncryptedSource"],
            json!(encrypted),
            "each curve point has its own original marker"
        );
        let point = proof::at(slot, "/value")?;
        proof::present(point, "sourceType", &json!(POINT), Some(encrypted))?;
        for (coordinate, value) in [("x", x), ("y", y)] {
            proof::present(
                point,
                coordinate,
                &json!({"rawBits": format!("{:08x}", value.to_bits()), "finite": value}),
                Some(encrypted),
            )?;
        }
        additional_member(point, "fixturePoint", encrypted)?;
    }
    Ok(())
}

fn additional_member(row: &Value, name: &str, encrypted: bool) -> TestResult {
    let root = proof::rows(row, "/additionalFields/roots")?
        .iter()
        .find(|root| root.pointer("/name/value").and_then(Value::as_str) == Some(name))
        .ok_or("original additional field")?;
    proof::present(root, "name", &json!(name), Some(encrypted))?;
    Ok(())
}

fn export(
    fixture: &DataForgeBuilder,
    origin: DcbOrigin,
    filter: Option<&Filter>,
) -> TestResult<Value> {
    let (_dir, archive) = common::open(fixture, origin)?;
    let options = CatalogOptions {
        filter,
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    Ok(serde_json::from_slice(
        build_catalog(&archive, &options)?.json(),
    )?)
}

fn database(matching_declaration: bool) -> DataForgeBuilder {
    let declaration = if matching_declaration {
        MODIFIER
    } else {
        "FixtureWrongLink"
    };

    p4kx_testkit::entity_schema(DataForgeBuilder::new().version(8))
        .structure(TestStruct::new("FixtureWrongLink"))
        .structure(
            TestStruct::new(TURRET)
                .parent(BASE)
                .field(
                    "healthModifierRecord",
                    TestType::ReferenceTo(declaration.to_owned()),
                )
                .field("fixtureTurret", TestType::Bool),
        )
        .structure(
            TestStruct::new(MODIFIER)
                .field("damageMovementModifier", TestType::Class(CURVE.to_owned()))
                .field("fixtureDefinition", TestType::Bool),
        )
        .structure(
            TestStruct::new(CURVE)
                .array("points", TestType::Class(POINT.to_owned()))
                .field("useLUT", TestType::Bool)
                .field("fixtureCurve", TestType::Bool),
        )
        .structure(
            TestStruct::new(POINT)
                .field("x", TestType::Single)
                .field("y", TestType::Single)
                .field("fixturePoint", TestType::Bool),
        )
        .record(
            TestEntity::new(OWNER, "FixtureTurret", OWNER_PATH)
                .component(turret(TestValue::Reference(DEFINITION)))
                .component(turret(TestValue::Null))
                .component(turret(TestValue::Reference(MISSING)))
                .record(),
        )
        .record(definition(DEFINITION, MODIFIER, DEFINITION_PATH))
        .record(definition(DEFINITION, MODIFIER, DUPLICATE_PATH))
        .record(definition(STANDALONE, MODIFIER, STANDALONE_PATH))
}

fn derived_database() -> DataForgeBuilder {
    database(true)
        .structure(TestStruct::new(DERIVED).parent(MODIFIER))
        .record(
            TestEntity::new(DERIVED_OWNER, "FixtureDerivedTurret", DERIVED_OWNER_PATH)
                .component(turret(TestValue::Reference(DERIVED_TARGET)))
                .record(),
        )
        .record(definition(
            DERIVED_TARGET,
            DERIVED,
            "libs/foundry/records/fixtures/modifier_derived.xml",
        ))
}

fn turret(reference: TestValue) -> TestValue {
    TestValue::Struct(
        TestInstance::new(TURRET)
            .value("healthModifierRecord", reference)
            .value("fixtureTurret", TestValue::Bool(true)),
    )
}

fn definition(guid: u128, source_type: &str, path: &str) -> TestRecord {
    let point = |x, y| {
        TestValue::Struct(
            TestInstance::new(POINT)
                .value("x", TestValue::Single(x))
                .value("y", TestValue::Single(y))
                .value("fixturePoint", TestValue::Bool(true)),
        )
    };
    let curve = TestInstance::new(CURVE)
        .value(
            "points",
            TestValue::Array(vec![point(0.0, 4.5), point(-1.0, -0.0)]),
        )
        .value("useLUT", TestValue::Bool(true))
        .value("fixtureCurve", TestValue::Bool(true));
    TestRecord::new(guid, source_type, source_type, path)
        .value("damageMovementModifier", TestValue::Struct(curve))
        .value("fixtureDefinition", TestValue::Bool(true))
}
