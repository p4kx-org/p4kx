// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::Kind;
use crate::export_diagnostic::{ExportDiagnosticCode, ExportSourceRole};
use crate::export_ledger::SourceDisposition;
use crate::mission_index::MissionIndex;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const SOURCE: u128 = 0x6a00_0000_0000_4000_8000_0000_0000_0001;
const WRONG_LOCALITY: &str = "libs/foundry/records/missiondata/pu_missionlocality/unrelated.xml";

fn unreadable_dependencies() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let generic = "Broken";
    let locality = Kind::MissionLocalities.records().type_name;
    let mut spec =
        [generic, locality]
            .into_iter()
            .fold(DataForgeBuilder::new(), |spec, type_name| {
                spec.structure(
                    TestStruct::new(type_name)
                        .field("value", TestType::Int32)
                        .declared_size(6),
                )
            });
    let families = [
        ("contracts/contracttemplates", generic),
        ("missiontype", generic),
        ("missiondata/pu_organizations", generic),
        ("reputation/rewards/missionrewards_reputation", generic),
        ("missiondata/pu_missionlocality", locality),
        ("missionbroker", generic),
        ("factions/factionreputation", generic),
    ];
    let unrelated = SOURCE + u128::try_from(families.len())?;
    for (index, (prefix, type_name)) in families.into_iter().enumerate() {
        spec = spec.record(TestRecord::new(
            SOURCE + u128::try_from(index)?,
            type_name,
            format!("{type_name}.Synthetic{index}"),
            format!("libs/foundry/records/{prefix}/synthetic.xml"),
        ));
    }
    Ok(spec
        .record(TestRecord::new(
            unrelated,
            generic,
            "Broken.Unrelated",
            WRONG_LOCALITY,
        ))
        .build())
}

#[test]
fn all_seven_unreadable_dependency_families_are_retained() -> TestResult {
    let bytes = unreadable_dependencies()?;
    let forge = DataForge::open(&bytes)?;
    let mut index = MissionIndex::read(&forge, None)?;
    let (_, failures, ledger) = index.take_template_results();
    assert_eq!(
        failures.len(),
        7,
        "no dependency read error is discarded by Result::ok"
    );
    let locality = Kind::MissionLocalities.records();
    assert!(
        failures.iter().any(|failure| failure
            .record()
            .type_name()
            .is_ok_and(|name| name == locality.type_name)),
        "the malformed canonical locality is a retained Contracts dependency"
    );
    assert!(
        failures.iter().all(|failure| failure
            .record()
            .record_path()
            .is_ok_and(|path| path != WRONG_LOCALITY)),
        "a different type in the locality folder is outside the dependency scope"
    );
    assert_eq!(
        ledger
            .sources
            .iter()
            .filter(|source| source.kind == Some(Kind::Contracts))
            .count(),
        7,
        "each actual dependency Record has its own physical source observation"
    );
    assert_eq!(
        ledger
            .sources
            .iter()
            .filter(|source| source.role == ExportSourceRole::DerivedInput)
            .count(),
        4,
        "the unreadable template is also an actual input of all four pool/query projections"
    );
    assert!(
        ledger
            .sources
            .iter()
            .all(|source| source.disposition == SourceDisposition::ReadFailed),
        "canonical failures are not relabeled typed projection failures"
    );
    assert!(
        ledger.field_inspections.is_empty(),
        "failed canonical reads have no fabricated field proofs"
    );
    assert!(
        ledger.inventories.is_empty(),
        "Index completion does not prematurely complete global pool inventories"
    );
    Ok(())
}

fn brokers_across_read_windows(window_size: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let home = SOURCE + 1;
    let departure = SOURCE + 2;
    let mut spec = DataForgeBuilder::new()
        .structure(TestStruct::new("Helper"))
        .structure(TestStruct::new("Broker").field("type", TestType::Reference))
        .record(
            TestRecord::new(
                SOURCE,
                "Broker",
                "Broker.First",
                "libs/foundry/records/missionbroker/first.xml",
            )
            .value("type", TestValue::Reference(home)),
        )
        .record(TestRecord::new(
            home,
            "Helper",
            "Helper.Home",
            "helper/home.xml",
        ));
    for position in 0..window_size {
        spec = spec.record(TestRecord::new(
            SOURCE + 3 + u128::try_from(position)?,
            "Helper",
            format!("Helper.Unselected{position}"),
            format!("helper/unselected{position}.xml"),
        ));
    }
    Ok(spec
        .record(TestRecord::new(
            departure,
            "Helper",
            "Helper.Departure",
            "helper/departure.xml",
        ))
        .record(
            TestRecord::new(
                SOURCE,
                "Broker",
                "Broker.Last",
                "libs/foundry/records/missionbroker/last.xml",
            )
            .value("type", TestValue::Reference(departure)),
        )
        .build())
}

#[test]
fn duplicate_brokers_keep_definition_order_across_read_windows() -> TestResult {
    let pool = rayon::ThreadPoolBuilder::new().num_threads(1).build()?;
    let window_size = pool.install(|| crate::record_read::window_size(usize::MAX));
    let bytes = brokers_across_read_windows(window_size)?;
    let forge = DataForge::open(&bytes)?;
    let guid = forge.records().next().ok_or("first broker fixture")?.guid();
    let selected_type = forge
        .records()
        .nth(window_size + 2)
        .ok_or("departure fixture")?
        .guid();
    let mut index = pool.install(|| MissionIndex::read(&forge, None))?;
    let broker = index.broker(guid).ok_or("indexed broker")?;
    assert_eq!(
        broker.mission_type,
        Some(selected_type),
        "the later selected definition replaces the earlier broker across windows"
    );
    let (_, failures, ledger) = index.take_template_results();
    assert!(
        failures.is_empty(),
        "both source brokers decode successfully"
    );
    assert_eq!(
        ledger.sources.len(),
        2,
        "each selected broker retains its original physical source"
    );
    Ok(())
}

#[test]
fn a_missing_optional_reward_amount_completes_a_zero_row_projection() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Reward"))
        .record(TestRecord::new(
            SOURCE,
            "Reward",
            "Reward.Synthetic",
            "libs/foundry/records/reputation/rewards/missionrewards_reputation/synthetic.xml",
        ))
        .build();
    let forge = DataForge::open(&bytes)?;
    let guid = forge
        .records()
        .next()
        .ok_or("reward fixture missing")?
        .guid();
    let mut index = MissionIndex::read(&forge, None)?;
    assert_eq!(
        index.reward(guid),
        None,
        "the known optional amount does not invent an output value"
    );
    let (_, failures, ledger) = index.take_template_results();
    assert!(
        failures.is_empty(),
        "the successful zero-row branch is not a canonical failure"
    );
    assert_eq!(
        ledger
            .sources
            .first()
            .ok_or("reward source outcome missing")?
            .disposition,
        SourceDisposition::Projected,
        "the actual successful optional branch is recorded"
    );
    assert_eq!(
        ledger.field_inspections.len(),
        1,
        "the source was inspected before optional projection helpers"
    );
    Ok(())
}

fn unknown_condition() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("ContractTemplate")
                .array("contractProperties", TestType::Class("Property".to_owned())),
        )
        .structure(
            TestStruct::new("Property")
                .field("missionVariableName", TestType::String)
                .field("value", TestType::StrongPointer("Value".to_owned())),
        )
        .structure(TestStruct::new("Value"))
        .structure(
            TestStruct::new("MissionPropertyValue_Location")
                .parent("Value")
                .array(
                    "matchConditions",
                    TestType::StrongPointer("Condition".to_owned()),
                ),
        )
        .structure(TestStruct::new("Condition"))
        .structure(TestStruct::new("DataSetMatchCondition_Synthetic").parent("Condition"))
        .record(
            TestRecord::new(
                SOURCE,
                "ContractTemplate",
                "ContractTemplate.Synthetic",
                "libs/foundry/records/contracts/contracttemplates/synthetic.xml",
            )
            .value(
                "contractProperties",
                TestValue::Array(vec![TestValue::Struct(
                    TestInstance::new("Property")
                        .value("missionVariableName", TestValue::text("synthetic"))
                        .value(
                            "value",
                            TestValue::Struct(
                                TestInstance::new("MissionPropertyValue_Location").value(
                                    "matchConditions",
                                    TestValue::Array(vec![TestValue::Struct(TestInstance::new(
                                        "DataSetMatchCondition_Synthetic",
                                    ))]),
                                ),
                            ),
                        ),
                )]),
            ),
        )
        .build()
}

#[test]
fn an_unknown_template_condition_is_a_typed_projection_failure() -> TestResult {
    let bytes = unknown_condition();
    let forge = DataForge::open(&bytes)?;
    let mut index = MissionIndex::read(&forge, None)?;
    let (_, failures, ledger) = index.take_template_results();
    assert_eq!(
        failures.len(),
        1,
        "the typed failure retains its compatibility diagnostic"
    );
    assert_eq!(
        ledger.sources.len(),
        5,
        "the actual template projection affects Contracts and four derived input kinds"
    );
    assert!(
        ledger.sources.iter().all(|source| source.disposition
            == SourceDisposition::ProjectionFailed
            && source.diagnosis().code == Some(ExportDiagnosticCode::UnsupportedValue)),
        "readable unsupported source structs retain their typed category"
    );
    assert_eq!(
        ledger.field_inspections.len(),
        1,
        "one canonical walk proves all related input roles"
    );
    assert_eq!(
        ledger
            .sources
            .first()
            .ok_or("source missing")?
            .diagnosis()
            .field
            .as_deref(),
        Some("location condition"),
        "the typed field name is preserved without parsing a reason string"
    );
    Ok(())
}

#[test]
fn duplicate_tag_guids_are_distinct_read_and_skip_sources() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Tag").field("tagName", TestType::String))
        .record(
            TestRecord::new(SOURCE, "Tag", "Tag.First", "synthetic/first.xml")
                .value("tagName", TestValue::text("first")),
        )
        .record(
            TestRecord::new(SOURCE, "Tag", "Tag.Second", "synthetic/second.xml")
                .value("tagName", TestValue::text("second")),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let mut index = MissionIndex::read(&forge, None)?;
    let (_, failures, ledger) = index.take_template_results();
    assert!(
        failures.is_empty(),
        "the established first-wins duplicate policy is an intentional skip"
    );
    let first = ledger
        .sources
        .iter()
        .find(|source| source.record_index == 0)
        .ok_or("first physical Record missing")?;
    let second = ledger
        .sources
        .iter()
        .find(|source| source.record_index == 1)
        .ok_or("second physical Record missing")?;
    assert_eq!(
        first.disposition,
        SourceDisposition::Projected,
        "the first physical Tag was projected"
    );
    assert_eq!(
        second.disposition,
        SourceDisposition::SkippedAfterRead,
        "the second physical Tag was read and intentionally skipped"
    );
    assert_eq!(
        first.guid, second.guid,
        "the test exercises an actual GUID alias"
    );
    assert_eq!(
        ledger.field_inspections.len(),
        2,
        "the two actual physical reads have distinct canonical proofs"
    );
    Ok(())
}
