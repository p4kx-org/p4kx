// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_extraction::Filter;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

const COMPONENT: &str = "SJumpPointParams";
const BASE: &str = "FixtureComponent";
const OWNER: &str = "EntityClassDefinition";
const SIZE: &str = "LandingPadSize";
const PUSH_AREA: &str = "SJumpPointPushAreaParams";
const EFFECT: &str = "JumpPointEffectParams";
const ROWS: &str = "/configurations/jumpPointConfigurations/rows";

#[test]
fn jump_point_configurations_preserve_physical_owner_and_native_fields() -> TestResult {
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&fixture(), origin)?;
        let filter = Filter::new("*/point");
        let mut options = common::options();
        options.filter = Some(&filter);
        let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
        let [point] = proof::rows(&catalog, ROWS)? else {
            return Err("only the admitted owner contributes a jump-point component".into());
        };
        check_point(&catalog, point, encrypted)?;
    }
    Ok(())
}

fn check_point(catalog: &Value, point: &Value, encrypted: bool) -> TestResult {
    proof::present(point, "sourceType", &json!(COMPONENT), Some(encrypted))?;
    proof::present(point, "ownerGuid", &json!(common::guid(1)), Some(encrypted))?;
    proof::present(point, "componentOrdinal", &json!(1), Some(encrypted))?;
    let fuel = proof::present(
        point,
        "requiredFuel",
        &json!({"finite": 3.25, "rawBits": "40500000"}),
        Some(encrypted),
    )?;
    assert!(
        !proof::uses(catalog, fuel)?.is_empty(),
        "fuel retains physical field evidence"
    );
    proof::present(
        point,
        "linkingRange",
        &json!({"finite": null, "rawBits": "7fc01234"}),
        Some(encrypted),
    )?;
    proof::present(point, "jumpGateSuperGuid", &json!(""), Some(encrypted))?;
    proof::cell(point, "pushAreaRadarGeometryName", "absent")?;
    let link = proof::at(proof::cell(point, "largestShipSize", "present")?, "/value")?;
    proof::present(link, "guid", &json!(common::guid(3)), Some(encrypted))?;
    proof::present(link, "targetType", &json!(SIZE), Some(encrypted))?;
    check_owned_members(point, encrypted)
}

fn check_owned_members(point: &Value, encrypted: bool) -> TestResult {
    let push = proof::at(
        proof::cell(point, "entrancePushAreaParams", "present")?,
        "/value",
    )?;
    proof::present(
        push,
        "falloffRatio",
        &json!({"finite": 0.5, "rawBits": "3f000000"}),
        Some(encrypted),
    )?;
    let effects = proof::at(proof::cell(point, "effectParams", "present")?, "/value")?;
    proof::present(effects, "sourceType", &json!(EFFECT), Some(encrypted))?;
    let [root] = proof::rows(effects, "/additionalFields/roots")? else {
        return Err("the unknown authored effect field is preserved".into());
    };
    proof::present(root, "name", &json!("fixtureEffect"), Some(encrypted))?;
    Ok(())
}

fn fixture() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(OWNER).array("Components", TestType::StrongPointer(BASE.to_owned())),
        )
        .structure(TestStruct::new(BASE))
        .structure(component_schema())
        .structure(TestStruct::new(SIZE))
        .structure(TestStruct::new(PUSH_AREA).field("falloffRatio", TestType::Single))
        .structure(TestStruct::new(EFFECT).field("fixtureEffect", TestType::Bool))
        .record(
            TestRecord::new(
                1,
                OWNER,
                "EntityClassDefinition.Point",
                "fixtures/point.xml",
            )
            .value(
                "Components",
                TestValue::Array(vec![
                    TestValue::Struct(TestInstance::new(BASE)),
                    component(),
                ]),
            ),
        )
        .record(
            TestRecord::new(
                2,
                OWNER,
                "EntityClassDefinition.Omitted",
                "fixtures/omitted.xml",
            )
            .value("Components", TestValue::Array(vec![component()])),
        )
        .record(TestRecord::new(
            3,
            SIZE,
            "Fixture",
            "fixtures/landing_size.xml",
        ))
}

fn component_schema() -> TestStruct {
    TestStruct::new(COMPONENT)
        .parent(BASE)
        .field("requiredFuel", TestType::Single)
        .field("linkingRange", TestType::Single)
        .field("jumpGateSuperGuid", TestType::String)
        .field("largestShipSize", TestType::ReferenceTo(SIZE.to_owned()))
        .field(
            "entrancePushAreaParams",
            TestType::Class(PUSH_AREA.to_owned()),
        )
        .field("effectParams", TestType::Class(EFFECT.to_owned()))
}

fn component() -> TestValue {
    TestValue::Struct(
        TestInstance::new(COMPONENT)
            .value("requiredFuel", TestValue::Single(3.25))
            .value(
                "linkingRange",
                TestValue::Single(f32::from_bits(0x7fc0_1234)),
            )
            .value("jumpGateSuperGuid", TestValue::text(""))
            .value("largestShipSize", TestValue::Reference(3))
            .value(
                "entrancePushAreaParams",
                TestValue::Struct(
                    TestInstance::new(PUSH_AREA).value("falloffRatio", TestValue::Single(0.5)),
                ),
            )
            .value(
                "effectParams",
                TestValue::Struct(
                    TestInstance::new(EFFECT).value("fixtureEffect", TestValue::Bool(true)),
                ),
            ),
    )
}
