// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

use super::TestResult;

const ROOT: &str = "ItemRecoveryConfigurationParams";
const ECONOMY: &str = "ItemRecoveryEconomyParams";
const OVERRIDE: &str = "ItemRecoveryOverrideGroupDef";
const NOTIFICATIONS: &str = "ItemRecoveryNotificationParams";
const BASE_CONDITION: &str = "ItemRecoverySetConditionDef";
const CLASS_CONDITION: &str = "ItemRecoveryCondition_EntityClass";
const POLICY_PATH: &str = "libs/foundry/records/entitlementpolicies/";
const OWNER: u128 = 0x10;
const ENTITY: u128 = 0x20;
const TEXT_KEY: &str = "fixture_recovery_title";
pub(super) const TEXT: &str = "Fixture recovery title";

pub(super) fn damaged_database() -> DataForgeBuilder {
    source_types()
        .record(recovery_record("aardvark.xml", OWNER, 0.5).with_instance_index(u16::MAX))
        .record(outside_policy())
        .record(entity())
        .record(recovery_record("alpha.xml", OWNER, 0.25))
        .record(recovery_record("alpha.xml", OWNER, 0.125))
}

pub(super) fn outside_database() -> DataForgeBuilder {
    source_types().record(outside_policy())
}

fn outside_policy() -> TestRecord {
    TestRecord::new(0x40, ROOT, "Outside", "synthetic/foreign.xml").with_instance_index(u16::MAX)
}

pub(super) fn full_database() -> DataForgeBuilder {
    source_types()
        .record(recovery_record("zeta.xml", 0x12, 0.5))
        .record(recovery_record("alpha.xml", OWNER, 0.25))
        .record(entity())
}

fn source_types() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("EntityClassDefinition"))
        .structure(TestStruct::new(BASE_CONDITION))
        .structure(
            TestStruct::new(CLASS_CONDITION)
                .parent(BASE_CONDITION)
                .field(
                    "classDef",
                    TestType::ReferenceTo("EntityClassDefinition".to_owned()),
                ),
        )
        .structure(TestStruct::new(NOTIFICATIONS).field("itemBricked_Title", TestType::Locale))
        .structure(
            TestStruct::new(OVERRIDE)
                .array(
                    "classes",
                    TestType::ReferenceTo("EntityClassDefinition".to_owned()),
                )
                .field("claimCostMultiplier", TestType::Single),
        )
        .structure(
            TestStruct::new(ECONOMY)
                .field("claimCostMultiplier", TestType::Single)
                .array("overrides", TestType::Class(OVERRIDE.to_owned())),
        )
        .structure(
            TestStruct::new(ROOT)
                .field("economyParams", TestType::Class(ECONOMY.to_owned()))
                .field(
                    "notificationParams",
                    TestType::Class(NOTIFICATIONS.to_owned()),
                )
                .array(
                    "Insurable",
                    TestType::StrongPointer(BASE_CONDITION.to_owned()),
                ),
        )
}

fn entity() -> TestRecord {
    TestRecord::new(
        ENTITY,
        "EntityClassDefinition",
        "EntityFixture",
        "synthetic/entity.xml",
    )
}

fn recovery_record(name: &str, guid: u128, multiplier: f32) -> TestRecord {
    let group = TestInstance::new(OVERRIDE)
        .value(
            "classes",
            TestValue::Array(vec![TestValue::Reference(ENTITY), TestValue::Null]),
        )
        .value("claimCostMultiplier", TestValue::Single(multiplier));
    let economy = TestInstance::new(ECONOMY)
        .value("claimCostMultiplier", TestValue::Single(multiplier))
        .value(
            "overrides",
            TestValue::Array(vec![TestValue::Struct(group)]),
        );
    let notifications =
        TestInstance::new(NOTIFICATIONS).value("itemBricked_Title", TestValue::text(TEXT_KEY));
    let condition =
        TestInstance::new(CLASS_CONDITION).value("classDef", TestValue::Reference(ENTITY));
    TestRecord::new(guid, ROOT, name, format!("{POLICY_PATH}{name}"))
        .value("economyParams", TestValue::Struct(economy))
        .value("notificationParams", TestValue::Struct(notifications))
        .value(
            "Insurable",
            TestValue::Array(vec![TestValue::Struct(condition), TestValue::Null]),
        )
}

pub(super) fn archive(
    builder: &DataForgeBuilder,
    with_english: bool,
) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("item-recovery-fixture.p4k");
    let mut archive = ArchiveBuilder::v2()
        .entry(TestEntry::stored("Data/Game2.dcb", builder.build()).encrypted())
        .entry(TestEntry::stored(
            "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml",
            b"<LayerBackups/>",
        ));
    if with_english {
        archive = archive.entry(TestEntry::stored(
            "Data/Localization/english/global.ini",
            format!("{TEXT_KEY}={TEXT}\n"),
        ));
    }
    archive.write_to(&path)?;
    Ok((
        folder,
        Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?,
    ))
}
