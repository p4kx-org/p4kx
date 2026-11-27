// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue, write_build_manifest,
};

use crate::commodity_v3_fixture::TestResult;
use crate::crafting_detail_fixture::{self, CONTRACT_ID, POOL};

pub(super) const LOCATION: u128 = 0x1165_0010;
pub(super) const REWARD_OWNER: u128 = 0x1165_0011;
pub(super) const BROKEN_OWNER: u128 = 0x1165_0012;
pub(super) const MISSING_OWNER: u128 = 0x1165_0013;

pub(super) fn archive(
    change: &str,
    disabled: bool,
    encrypted: bool,
) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let channel = folder.path().join("PTU");
    std::fs::create_dir(&channel)?;
    write_build_manifest(&channel, "sc-alpha-5.0.0", "5.0.0", change, change)?;
    let path = channel.join("Data.p4k");
    let dcb = TestEntry::stored("Data/Game2.dcb", database(disabled).build());
    let dcb = if encrypted { dcb.encrypted() } else { dcb };
    ArchiveBuilder::new().entry(dcb).write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((folder, archive))
}

fn database(disabled: bool) -> DataForgeBuilder {
    let database = crate::catalog_loadouts::catalog_item_alignment::consumer_ports();
    crafting_detail_fixture::with_crafting_detail(database)
        .structure(TestStruct::new("MissionLocationData").field("disabled", TestType::Bool))
        .structure(TestStruct::new("MissionLocationTemplate").field(
            "locationData",
            TestType::Class("MissionLocationData".to_owned()),
        ))
        .record(
            TestRecord::new(
                LOCATION,
                "MissionLocationTemplate",
                "MissionLocationTemplate.FixtureDepot",
                "libs/foundry/records/missiondata/pu_locations/fixture/depot.xml",
            )
            .value(
                "locationData",
                TestValue::Struct(
                    TestInstance::new("MissionLocationData")
                        .value("disabled", TestValue::Bool(disabled)),
                ),
            ),
        )
        .structure(
            TestStruct::new("FixtureRewardSource")
                .field("id", TestType::Guid)
                .field("debugName", TestType::String)
                .field("blueprintPool", TestType::Reference)
                .field("chance", TestType::Single),
        )
        .record(
            TestRecord::new(
                REWARD_OWNER,
                "FixtureRewardSource",
                "FixtureRewardSource.Depot",
                "libs/foundry/records/fixture/depot_reward.xml",
            )
            .value("id", TestValue::Guid(CONTRACT_ID))
            .value("debugName", TestValue::text("Depot reward"))
            .value("blueprintPool", TestValue::Reference(POOL))
            .value("chance", TestValue::Single(0.5)),
        )
        .structure(
            TestStruct::new("FixtureBrokenRewardSource")
                .field("outside", TestType::UInt64)
                .declared_size(1),
        )
        .record(TestRecord::new(
            BROKEN_OWNER,
            "FixtureBrokenRewardSource",
            "FixtureBrokenRewardSource.Outside",
            "libs/foundry/records/fixture/outside.xml",
        ))
}
