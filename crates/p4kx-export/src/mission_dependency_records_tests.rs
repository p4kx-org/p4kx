// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::MissionIndex;
use p4kx_extraction::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const REWARD: u128 = 0x5500_0000_0000_4000_8000_0000_0000_0001;

#[test]
fn explicit_dependency_records_keep_nominal_reward_order_and_actual_failures() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let records = forge.records().take(3).collect::<Vec<_>>();
    let nominal = records.first().ok_or("the original first reward")?.guid();
    let mut selected = MissionIndex::read_candidate_records(&forge, records.into_iter(), None)?;
    let mut full = MissionIndex::read(&forge, None)?;
    assert_eq!(
        selected.reward(nominal),
        Some(9),
        "last usable physical amount"
    );
    assert_eq!(
        selected.reward(nominal),
        full.reward(nominal),
        "the nominal rule is shared"
    );
    let (_, selected_failures, selected_ledger) = selected.take_template_results();
    let (_, full_failures, _) = full.take_template_results();
    assert_eq!(
        selected_failures.len(),
        1,
        "the selected unreadable candidate stays visible"
    );
    assert_eq!(
        full_failures.len(),
        2,
        "the complete reader also observes its unrelated failure"
    );
    assert_eq!(
        selected_failures[0].record().index(),
        2,
        "the actual failed physical candidate"
    );
    assert!(
        selected_ledger
            .sources
            .iter()
            .all(|source| source.record_index < 3),
        "an unrelated dependency body is outside the explicit read scope"
    );
    Ok(())
}

fn database() -> Vec<u8> {
    let prefix = "libs/foundry/records/reputation/rewards/missionrewards_reputation/";
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Reward").field("reputationAmount", TestType::Int64))
        .structure(
            TestStruct::new("Broken")
                .field("value", TestType::Int32)
                .declared_size(6),
        )
        .record(
            TestRecord::new(
                REWARD,
                "Reward",
                "Reward.First",
                format!("{prefix}first.xml"),
            )
            .value("reputationAmount", TestValue::Int64(5)),
        )
        .record(
            TestRecord::new(REWARD, "Reward", "Reward.Last", format!("{prefix}last.xml"))
                .value("reputationAmount", TestValue::Int64(9)),
        )
        .record(TestRecord::new(
            REWARD,
            "Broken",
            "Broken.Candidate",
            format!("{prefix}broken.xml"),
        ))
        .record(TestRecord::new(
            REWARD + 1,
            "Broken",
            "Broken.Other",
            "libs/foundry/records/missionbroker/other.xml",
        ))
        .build()
}
