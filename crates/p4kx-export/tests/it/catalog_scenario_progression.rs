// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::build_catalog_from;
use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_extraction::DataForgeEntry;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use super::{TestResult, build, guid, proof};
use crate::commodity_v3_fixture::{self as common, DcbOrigin};
use crate::reputation_fixture::{FACTION, LEGACY_FACTION, MISSING, PLAIN_A, PLAIN_B, SCENARIO};

#[test]
fn equal_guid_and_path_scenarios_keep_distinct_physical_owners_and_numbers() -> TestResult {
    let (_folder, archive) = common::open(&scenario_aliases(), DcbOrigin::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let built = build_catalog_from(&archive, &entry, common::options().with_full_provenance())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let [first, second] = proof::rows(&catalog, "/contractScenarios")? else {
        return Err("exactly two physical owners share the same GUID and path".into());
    };

    for (row, index, threshold_bits) in [(first, 0, "3f800000"), (second, 1, "40000000")] {
        proof::present(row, "guid", &json!(guid(SCENARIO)), Some(true))?;
        proof::present(row, "recordIndex", &json!(index), Some(true))?;
        assert_eq!(
            proof::at(
                row,
                "/factionRewardTiers/value/0/value/progressions/value/0/value/minMultiplierThreshold/value/rawBits"
            )?,
            threshold_bits,
            "each physical owner keeps its own original multiplier"
        );
        full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?
            .with_contract_scenarios(
                CatalogRecordSelection::RecordIndex(index),
                None,
                |snapshot| -> TestResult {
                    let [selected] = snapshot.rows.as_slice() else {
                        return Err("one physical scenario selected by index".into());
                    };
                    assert_eq!(
                        &serde_json::to_value(selected)?,
                        row,
                        "the scoped SDK returns the same exact physical Catalog row"
                    );
                    assert!(
                        snapshot.coverage.incomplete_fields.is_empty(),
                        "all original multiplier and identity fields remain complete"
                    );
                    Ok(())
                },
            )??;
    }
    proof::closure(&catalog)
}

fn scenario_aliases() -> DataForgeBuilder {
    const ROOT: &str = "ScenarioProgress";
    const FACTION_TIERS: &str = "SScenarioProgressRewardsTiers";
    const PROGRESSION: &str = "STierProgressions";
    const PATH: &str = "libs/foundry/records/contracts/contractscenarios/shared_alias.xml";
    let mut builder = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(ROOT).array(
            "factionRewardTiers",
            TestType::Class(FACTION_TIERS.to_owned()),
        ))
        .structure(
            TestStruct::new(FACTION_TIERS)
                .array("tierProgressions", TestType::Class(PROGRESSION.to_owned())),
        )
        .structure(TestStruct::new(PROGRESSION).field("minMultiplierThreshold", TestType::Single));
    for (name, multiplier) in [("SharedAlias1", 1.0), ("SharedAlias2", 2.0)] {
        let progression = TestInstance::new(PROGRESSION)
            .value("minMultiplierThreshold", TestValue::Single(multiplier));
        let tiers = TestInstance::new(FACTION_TIERS).value(
            "tierProgressions",
            TestValue::Array(vec![TestValue::Struct(progression)]),
        );
        builder = builder.record(TestRecord::new(SCENARIO, ROOT, name, PATH).value(
            "factionRewardTiers",
            TestValue::Array(vec![TestValue::Struct(tiers)]),
        ));
    }
    builder
}

#[test]
fn a_scenario_lists_the_original_ordered_tiers_and_completion_members() -> TestResult {
    let (_, catalog) = build(None)?;
    let expected = json!(guid(SCENARIO));
    let scenario = proof::rows(&catalog, "/contractScenarios")?
        .iter()
        .find(|row| row.pointer("/guid/value") == Some(&expected))
        .ok_or("the physical scenario progress Record")?;
    proof::present(
        scenario,
        "sourceType",
        &json!("ScenarioProgress"),
        Some(true),
    )?;
    let faction = proof::at(scenario, "/factionRewardTiers/value/0/value")?;
    assert_eq!(
        proof::at(faction, "/faction/value/guid/value")?,
        &json!(guid(LEGACY_FACTION)),
        "the original Faction Reference is retained"
    );
    assert_eq!(
        proof::at(faction, "/factionReputation/value/guid/value")?,
        &json!(guid(FACTION)),
        "the physically selected target supplies its original reputation Reference"
    );
    proof::present(
        faction,
        "factionName",
        &json!("faction_lawful_crusaderindustries"),
        Some(true),
    )?;
    proof::present(faction, "pointsAsPercent", &json!(false), Some(true))?;
    let progression = proof::at(faction, "/progressions/value/0/value")?;
    proof::present(
        progression,
        "completionType",
        &json!("CompletionType_CompletionTag"),
        Some(true),
    )?;
    assert_eq!(
        proof::at(progression, "/minMultiplierThreshold/value/rawBits")?,
        "3f800000",
        "the original IEEE multiplier bits survive"
    );
    proof::cell(progression, "periodMissionScenario", "null")?;
    check_scenario_tiers(progression)?;
    check_completion_pointer(progression)?;
    proof::closure(&catalog)
}

fn check_scenario_tiers(progression: &Value) -> TestResult {
    let [first, second] = proof::rows(progression, "/tiers/value")? else {
        return Err("two ordered original slots".into());
    };
    for (slot, points, badge) in [(first, 10_800, "ORS_Pistol"), (second, 18_000, "None")] {
        let tier = proof::at(slot, "/value")?;
        proof::present(
            tier,
            "minPoints",
            &json!({"type":"int32","value":points}),
            Some(true),
        )?;
        proof::present(tier, "badgeToAward", &json!(badge), Some(true))?;
        assert_eq!(
            proof::at(tier, "/globalPointsMultiplier/value/rawBits")?,
            "3f800000",
            "the exact reward multiplier survives"
        );
    }
    assert_eq!(
        proof::at(first, "/value/blueprintPools/value")?,
        &json!([]),
        "the authored empty list"
    );
    let pools = proof::rows(second, "/value/blueprintPools/value")?;
    let [resolved, dangling] = pools else {
        return Err("both original blueprint pool positions survive".into());
    };
    for (slot, target, status) in [
        (resolved, PLAIN_B, "present"),
        (dangling, MISSING, "dangling"),
    ] {
        assert_eq!(
            proof::at(slot, "/value/guid/value")?,
            &json!(guid(target)),
            "original pool reference GUID"
        );
        assert_eq!(
            proof::at(slot, "/value/status")?,
            status,
            "original target resolution"
        );
    }
    Ok(())
}

fn check_completion_pointer(progression: &Value) -> TestResult {
    let [declaration] = proof::rows(progression, "/authoredFields/roots")? else {
        return Err("the original completion declaration is retained once".into());
    };
    assert_eq!(
        proof::at(declaration, "/name/value")?,
        "completionType",
        "the authored field name"
    );
    assert_eq!(
        proof::at(declaration, "/declaration/dataType/value")?,
        "strongPointer",
        "the original owner stored a StrongPointer"
    );
    let nodes = proof::rows(progression, "/authoredFields/values")?;
    assert!(
        nodes.iter().any(|node| {
            node.pointer("/value/value/type").and_then(Value::as_str) == Some("recordReference")
                && node.pointer("/value/value/value/guid/value") == Some(&json!(guid(PLAIN_A)))
        }),
        "the reached authored completionTags Reference is retained"
    );
    assert!(
        nodes.iter().any(|node| {
            node.pointer("/value/value/value") == Some(&json!({"type":"int32","value":42}))
        }),
        "the uninterpreted completion member is retained"
    );
    Ok(())
}
