// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult, at, guid};

#[path = "catalog_location_auto_spawn.rs"]
mod auto_spawn;

#[path = "catalog_location_mission_module.rs"]
mod mission_module;

const TEMPLATE: u128 = 0x1074_0001;
const MISSING: u128 = 0x1074_00ff;

#[test]
fn disabled_location_keeps_all_authored_limits_and_security_reference() -> TestResult {
    let (_directory, archive) = common::open(&database(), DcbOrigin::Encrypted)?;
    let built = build_catalog(&archive, &common::options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let location = at(&catalog, "/missionLocationTemplates/0")?;
    assert_eq!(
        at(location, "/guid")?,
        &json!(guid(TEMPLATE)),
        "physical owner"
    );
    let config = at(location, "/configuration/value")?;
    assert_eq!(at(location, "/disabled")?, true, "authored disabled flag");
    for (index, maximum) in [0, -1].into_iter().enumerate() {
        let row = at(config, &format!("/missionLimits/value/{index}/value"))?;
        assert_eq!(
            at(row, "/maxMissionInstances/value")?,
            &json!(maximum),
            "original limit"
        );
        assert_eq!(
            at(row, "/maxMissionInstances/containsEncryptedSource")?,
            true,
            "per-limit origin"
        );
    }
    let link = at(config, "/defaultSecurityNetworkManifest/value")?;
    assert_eq!(
        at(link, "/guid/value")?,
        &json!(guid(MISSING)),
        "original unresolved GUID"
    );
    assert_eq!(at(link, "/status")?, "dangling", "source resolution state");
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    if let Err(error) = validator.validate(&catalog) {
        return Err(format!("{}: {}", error.instance_path(), error).into());
    }
    crate::commodity_v3_source_inventory::reference_closure(&catalog)
}

fn database() -> DataForgeBuilder {
    let limit_type = "LocationMissionLimit";
    let location_type = "MissionLocationData";
    let manifest_type = "SecurityNetworkManifest";
    let owner_type = "MissionLocationTemplate";
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Tag"))
        .structure(TestStruct::new(manifest_type))
        .structure(
            TestStruct::new(limit_type)
                .field("tag", TestType::ReferenceTo("Tag".to_owned()))
                .field("autoConsumeIfPlayerAtLocation", TestType::Bool)
                .field("maxMissionInstances", TestType::Int32),
        )
        .structure(
            TestStruct::new(location_type)
                .field("disabled", TestType::Bool)
                .field("isSecurityNetworkHost", TestType::Bool)
                .array("missionLimits", TestType::Class(limit_type.to_owned()))
                .field(
                    "defaultSecurityNetworkManifest",
                    TestType::ReferenceTo(manifest_type.to_owned()),
                ),
        )
        .structure(
            TestStruct::new(owner_type)
                .field("locationData", TestType::Class(location_type.to_owned())),
        )
        .record(
            TestRecord::new(
                TEMPLATE,
                owner_type,
                "MissionLocationTemplate.Fixture",
                "libs/foundry/records/missiondata/pu_locations/fixture/owner.xml",
            )
            .value(
                "locationData",
                TestValue::Struct(
                    TestInstance::new(location_type)
                        .value("disabled", TestValue::Bool(true))
                        .value("isSecurityNetworkHost", TestValue::Bool(false))
                        .value(
                            "missionLimits",
                            TestValue::Array(vec![limit(limit_type, 0), limit(limit_type, -1)]),
                        )
                        .value(
                            "defaultSecurityNetworkManifest",
                            TestValue::Reference(MISSING),
                        ),
                ),
            ),
        )
}

fn limit(source_type: &str, maximum: i32) -> TestValue {
    TestValue::Struct(
        TestInstance::new(source_type)
            .value("tag", TestValue::Null)
            .value("autoConsumeIfPlayerAtLocation", TestValue::Bool(false))
            .value("maxMissionInstances", TestValue::Int32(maximum)),
    )
}
