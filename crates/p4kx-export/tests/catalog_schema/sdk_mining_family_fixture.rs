// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

use super::TestResult;

const ELEMENT: u128 = 1;
const COMPOSITION: u128 = 2;
const REFINING: u128 = 3;
const GLOBAL: u128 = 4;
const PROVIDER: u128 = 5;
const ELEMENT_TYPE: &str = "MineableElement";
const COMPOSITION_TYPE: &str = "MineableComposition";
const PART_TYPE: &str = "MineableCompositionPart";
const REFINING_TYPE: &str = "RefiningProcess";
const SPEED_TYPE: &str = "ObservedRefiningSpeed";
const GLOBAL_TYPE: &str = "MiningGlobalParams";
const PROVIDER_TYPE: &str = "HarvestableProviderPreset";
const GROUP_TYPE: &str = "HarvestableElementGroup";

pub(super) fn open() -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("synthetic-mining.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database()).encrypted())
        .entry(TestEntry::stored(
            Language::English.global_ini_path(),
            b"fixture_deposit=Fixture Deposit\nfixture_refining=Fixture Refining\n",
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((folder, archive))
}

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .enumeration(SPEED_TYPE, ["Fast"])
        .structure(TestStruct::new(ELEMENT_TYPE).field("elementInstability", TestType::Single))
        .structure(
            TestStruct::new(COMPOSITION_TYPE)
                .field("depositName", TestType::Locale)
                .field("minimumDistinctElements", TestType::UInt16)
                .array("compositionArray", TestType::Class(PART_TYPE.to_owned())),
        )
        .structure(
            TestStruct::new(PART_TYPE)
                .field(
                    "mineableElement",
                    TestType::ReferenceTo(ELEMENT_TYPE.to_owned()),
                )
                .field("minPercentage", TestType::Single),
        )
        .structure(
            TestStruct::new(REFINING_TYPE)
                .field("processName", TestType::Locale)
                .field("refiningSpeed", TestType::Enum(SPEED_TYPE.to_owned())),
        )
        .structure(
            TestStruct::new(GLOBAL_TYPE).field("compositionMassMultiplier", TestType::Single),
        )
        .structure(
            TestStruct::new(PROVIDER_TYPE)
                .array("harvestableGroups", TestType::Class(GROUP_TYPE.to_owned())),
        )
        .structure(
            TestStruct::new(GROUP_TYPE)
                .field("groupName", TestType::String)
                .field("groupProbability", TestType::Single),
        )
        .record(
            record(ELEMENT, ELEMENT_TYPE, "mining/mineableelements/fixture")
                .value("elementInstability", TestValue::Single(0.25)),
        )
        .record(composition())
        .record(
            record(REFINING, REFINING_TYPE, "refiningprocess/fixture")
                .value("processName", TestValue::text("@fixture_refining"))
                .value("refiningSpeed", TestValue::text("Fast")),
        )
        .record(
            record(GLOBAL, GLOBAL_TYPE, "mining/miningglobalparams_fixture")
                .value("compositionMassMultiplier", TestValue::Single(1.25)),
        )
        .record(provider())
        .build()
}

fn composition() -> TestRecord {
    let part = TestInstance::new(PART_TYPE)
        .value("mineableElement", TestValue::Reference(ELEMENT))
        .value("minPercentage", TestValue::Single(20.0));
    record(
        COMPOSITION,
        COMPOSITION_TYPE,
        "mining/rockcompositionpresets/fixture",
    )
    .value("depositName", TestValue::text("@fixture_deposit"))
    .value("minimumDistinctElements", TestValue::UInt16(1))
    .value(
        "compositionArray",
        TestValue::Array(vec![TestValue::Struct(part)]),
    )
}

fn provider() -> TestRecord {
    let group = TestInstance::new(GROUP_TYPE)
        .value("groupName", TestValue::text("Fixture group"))
        .value("groupProbability", TestValue::Single(2.0));
    record(
        PROVIDER,
        PROVIDER_TYPE,
        "harvestable/providerpresets/fixture",
    )
    .value(
        "harvestableGroups",
        TestValue::Array(vec![TestValue::Struct(group)]),
    )
}

fn record(guid: u128, source_type: &str, path: &str) -> TestRecord {
    TestRecord::new(
        guid,
        source_type,
        format!("{source_type}.Fixture"),
        format!("libs/foundry/records/{path}.xml"),
    )
}
