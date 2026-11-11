// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selection, source state and link behavior of the canonical item-recovery tables.

use crate::insurance_fixture::{
    BARE, INSURED, RECOVERY, RECOVERY_PATH, build_from, database, diagnostic_from, entity,
    override_group, recovery_record, structures,
};

use std::error::Error;

use p4kx_export::Filter;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::health_origin_assertions as proof;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const OTHER: u128 = 0x6000_0000_0000_4000_8000_0000_0000_0006;
const BROKEN: u128 = 0x6000_0000_0000_4000_8000_0000_0000_0007;
const DANGLING: u128 = 0x6000_0000_0000_4000_8000_0000_0000_00ff;
const POLICIES: &str = "libs/foundry/records/entitlementpolicies/";

fn failure_guids(built: &p4kx_export::BuiltCatalog) -> Vec<String> {
    built
        .summary()
        .failures()
        .iter()
        .map(|failure| failure.guid().to_string())
        .collect()
}

#[test]
fn an_archive_without_the_record_has_null_and_no_failure() -> TestResult {
    let forge = structures(DataForgeBuilder::new().version(8));
    let (built, catalog) = build_from(&forge, None)?;
    assert_eq!(
        economy_rows(&catalog)?,
        &json!([]),
        "no selected readable recovery row"
    );
    assert_eq!(failure_guids(&built).len(), 0);
    Ok(())
}

#[test]
fn a_record_of_the_type_outside_the_policy_folder_is_not_selected() -> TestResult {
    let forge = structures(DataForgeBuilder::new().version(8)).record(recovery_record(
        OTHER,
        "Elsewhere",
        "libs/foundry/records/other/elsewhere",
        0.5,
        Vec::new(),
    ));
    let (_, catalog) = build_from(&forge, None)?;
    assert_eq!(
        economy_rows(&catalog)?,
        &json!([]),
        "no selected readable recovery row"
    );
    Ok(())
}

#[test]
fn a_filter_that_excludes_the_record_gives_null_without_a_failure() -> TestResult {
    let excluding = Filter::new("*/spaceships/*");
    let (built, catalog) = build_from(&database(), Some(&excluding))?;
    assert_eq!(
        economy_rows(&catalog)?,
        &json!([]),
        "no selected readable recovery row"
    );
    assert_eq!(failure_guids(&built).len(), 0);
    let including = Filter::new("*/entitlementpolicies/*");
    let (_, catalog) = build_from(&database(), Some(&including))?;
    assert_eq!(
        proof::at(recovery_row(&catalog)?, "/source/recordPath/value")?,
        RECOVERY_PATH,
        "the matching policy folder exposes its original recovery path"
    );
    Ok(())
}

#[test]
fn of_several_records_the_smallest_record_path_wins() -> TestResult {
    let forge = structures(DataForgeBuilder::new().version(8))
        .record(recovery_record(
            OTHER,
            "Zeta",
            &format!("{POLICIES}zeta"),
            0.5,
            Vec::new(),
        ))
        .record(recovery_record(
            RECOVERY,
            "Alpha",
            &format!("{POLICIES}alpha"),
            0.25,
            Vec::new(),
        ));
    let (_, catalog) = build_from(&forge, None)?;
    assert_eq!(
        proof::at(recovery_row(&catalog)?, "/source/className/value")?,
        "Alpha",
        "record path order selects Alpha"
    );
    assert_eq!(
        proof::at(legacy(&catalog)?, "/claimCostMultiplier/value/finite")?.as_f64(),
        Some(0.25),
        "selected Alpha multiplier remains exact"
    );
    Ok(())
}

fn broken_forge() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("ItemRecoveryConfigurationParams")
                .field("a", TestType::Int8)
                .declared_size(3),
        )
        .record(TestRecord::new(
            BROKEN,
            "ItemRecoveryConfigurationParams",
            "ItemRecoveryConfigurationParams.Broken",
            format!("{POLICIES}broken.xml"),
        ))
}

#[test]
fn an_unreadable_record_is_listed_as_a_failure_and_gives_null() -> TestResult {
    let (built, catalog) = diagnostic_from(&broken_forge(), None)?;
    assert_eq!(
        economy_rows(&catalog)?,
        &json!([]),
        "no selected readable recovery row"
    );
    assert_eq!(
        failure_guids(&built),
        ["60000000-0000-4000-8000-000000000007"]
    );
    Ok(())
}

#[test]
fn an_unreadable_record_outside_the_filter_is_no_failure() -> TestResult {
    let filter = Filter::new("*/spaceships/*");
    let (built, catalog) = build_from(&broken_forge(), Some(&filter))?;
    assert_eq!(
        economy_rows(&catalog)?,
        &json!([]),
        "no selected readable recovery row"
    );
    assert_eq!(failure_guids(&built).len(), 0);
    Ok(())
}

fn odd_economy() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("ItemRecoveryEconomyParams")
                .field("claimCostMultiplier", TestType::String)
                .field("baseDeliveryTime", TestType::Int32)
                .field("aUECperSecond", TestType::Double)
                .field("globalBrickTimer", TestType::Bool)
                .field("scalingPriceFloor", TestType::Single),
        )
        .structure(TestStruct::new("ItemRecoveryConfigurationParams").field(
            "economyParams",
            TestType::Class("ItemRecoveryEconomyParams".to_owned()),
        ))
        .record(
            TestRecord::new(
                RECOVERY,
                "ItemRecoveryConfigurationParams",
                "ItemRecoveryConfigurationParams.Odd",
                format!("{RECOVERY_PATH}.xml"),
            )
            .value(
                "economyParams",
                TestValue::Struct(
                    TestInstance::new("ItemRecoveryEconomyParams")
                        .value("claimCostMultiplier", TestValue::text("0.003"))
                        .value("baseDeliveryTime", TestValue::Int32(7))
                        .value("aUECperSecond", TestValue::Double(0.5))
                        .value("globalBrickTimer", TestValue::Bool(true))
                        .value("scalingPriceFloor", TestValue::Single(f32::NAN)),
                ),
            ),
        )
}

#[test]
fn invalid_missing_and_nonfinite_economy_fields_remain_distinct() -> TestResult {
    assert!(
        build_from(&odd_economy(), None).is_err(),
        "invalid populated fields prevent normal publication"
    );
    let (_, catalog) = diagnostic_from(&odd_economy(), None)?;
    let recovery = legacy(&catalog)?;
    assert_eq!(
        recovery["claimCostMultiplier"]["state"], "invalid",
        "a text"
    );
    assert_eq!(
        recovery["globalBrickTimer"]["state"], "invalid",
        "a boolean"
    );
    assert_eq!(
        recovery["scalingPriceFloor"]["state"], "present",
        "legal IEEE NaN"
    );
    assert_eq!(
        recovery["scalingPriceFloor"]["value"]["finite"],
        Value::Null,
        "no finite projection"
    );
    assert_eq!(
        recovery["maxExpeditedClaimCostMultiplier"]["state"],
        "absent"
    );
    assert_eq!(
        recovery["baseDeliveryTime"]["value"]["value"], 7,
        "exact integer"
    );
    assert_eq!(
        recovery["aUECperSecond"]["value"]["finite"].as_f64(),
        Some(0.5),
        "double"
    );
    assert_eq!(
        proof::at(economy(&catalog)?, "/escalatingClaimsParams/state")?,
        "absent",
        "no declaration adds escalating claims"
    );
    assert_eq!(
        proof::at(overrides(&catalog)?, "/state")?,
        "absent",
        "undeclared overrides remain absent"
    );
    Ok(())
}

#[test]
fn a_record_without_economy_params_keeps_explicit_absence() -> TestResult {
    let forge = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("ItemRecoveryConfigurationParams"))
        .record(TestRecord::new(
            RECOVERY,
            "ItemRecoveryConfigurationParams",
            "ItemRecoveryConfigurationParams.Bare",
            format!("{RECOVERY_PATH}.xml"),
        ));
    let (_, catalog) = build_from(&forge, None)?;
    let recovery = recovery_row(&catalog)?;
    assert_eq!(recovery["source"]["className"]["value"], "Bare");
    assert_eq!(recovery["economyParams"]["state"], "absent");
    assert_eq!(
        proof::at(overrides(&catalog)?, "/state")?,
        "absent",
        "undeclared overrides remain absent"
    );
    Ok(())
}

#[test]
fn original_class_links_retain_null_and_dangling_slots() -> TestResult {
    let group = override_group(
        1.0,
        vec![
            TestValue::Reference(INSURED),
            TestValue::Null,
            TestValue::Reference(DANGLING),
            TestValue::Reference(BARE),
        ],
    );
    let forge = database().record(recovery_record(
        OTHER,
        "Zzz",
        &format!("{POLICIES}zzz"),
        0.1,
        vec![group],
    ));
    let (_, catalog) = build_from(&forge, Some(&Filter::new("*/zzz")))?;
    let classes = proof::at(overrides(&catalog)?, "/value/0/value/classes/value")?;
    assert_eq!(
        classes[0]["value"]["guid"]["value"],
        "60000000-0000-4000-8000-000000000001"
    );
    assert_eq!(classes[1]["state"], "null");
    assert_eq!(classes[2]["value"]["status"], "dangling");
    assert_eq!(
        classes[2]["value"]["guid"]["value"],
        "60000000-0000-4000-8000-0000000000ff"
    );
    assert_eq!(
        classes[3]["value"]["guid"]["value"],
        "60000000-0000-4000-8000-000000000003"
    );
    Ok(())
}

#[test]
fn a_ship_without_insurance_parameters_has_every_field_null() -> TestResult {
    let forge = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("StaticEntityClassData"))
        .structure(TestStruct::new("SEntityInsuranceProperties").parent("StaticEntityClassData"))
        .structure(TestStruct::new("EntityClassDefinition").array(
            "StaticEntityClassData",
            TestType::StrongPointer("StaticEntityClassData".to_owned()),
        ))
        .record(entity(
            INSURED,
            "AEGS_Bare",
            "spaceships/aegs_bare",
            vec![TestValue::Struct(TestInstance::new(
                "SEntityInsuranceProperties",
            ))],
        ));
    let (_, catalog) = build_from(&forge, None)?;
    assert_eq!(
        catalog["ships"][0]["insurance"],
        json!({
            "restoreOnClaim": null,
            "shipEntityClassName": null,
            "baseWaitTimeMinutes": null,
            "mandatoryWaitTimeMinutes": null,
            "expediteFee": null
        })
    );
    Ok(())
}

fn economy_rows(catalog: &Value) -> TestResult<&Value> {
    proof::at(catalog, "/configurations/itemRecoveryEconomy/rows")
}

pub(super) fn recovery_row(catalog: &Value) -> TestResult<&Value> {
    proof::at(economy_rows(catalog)?, "/0")
}

pub(super) fn economy(catalog: &Value) -> TestResult<&Value> {
    proof::at(recovery_row(catalog)?, "/economyParams/value")
}

pub(super) fn legacy(catalog: &Value) -> TestResult<&Value> {
    proof::at(economy(catalog)?, "/legacy")
}

pub(super) fn overrides(catalog: &Value) -> TestResult<&Value> {
    proof::at(
        catalog,
        "/configurations/itemRecoveryOverrides/rows/0/overrides",
    )
}

#[path = "catalog_recovery_nested.rs"]
mod nested;
#[path = "catalog_recovery_policy.rs"]
mod policy;
