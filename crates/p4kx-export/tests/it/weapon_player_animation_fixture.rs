// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType, TestValue,
};

pub(super) const WEAPON: u32 = 1;
pub(super) const OTHER_ITEM: u32 = 2;

const COMPONENT_BASE: &str = "DataForgeComponentParams";
const ENTITY: &str = "EntityClassDefinition";
const ATTACHMENT: &str = "SAttachableComponentParams";
const ITEM: &str = "SItemDefinition";
const MOTION: &str = "SAnimatedCharacterComponentParams";

fn database_schema() -> DataForgeBuilder {
    use TestType::{Class, String as StringType, StrongPointer};
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(COMPONENT_BASE))
        .structure(
            TestStruct::new(ENTITY).array("Components", StrongPointer(COMPONENT_BASE.to_owned())),
        )
        .structure(
            TestStruct::new(ITEM)
                .field("Type", StringType)
                .field("Tags", StringType),
        )
        .structure(
            TestStruct::new(ATTACHMENT)
                .parent(COMPONENT_BASE)
                .field("AttachDef", Class(ITEM.to_owned())),
        )
        .structure(
            TestStruct::new(MOTION)
                .parent(COMPONENT_BASE)
                .field("actionController", StringType)
                .field("animDatabase3P", StringType),
        )
}

pub(super) fn database() -> Vec<u8> {
    database_with_player_assets("Animations/TestController.xml", "Animations/TestRoot.adb")
}

pub(super) fn database_with_player_assets(controller: &str, database: &str) -> Vec<u8> {
    database_schema()
        .record(
            TestRecord::new(
                0x1053,
                ENTITY,
                "EntityClassDefinition.Player",
                "synthetic/player.xml",
            )
            .value(
                "Components",
                TestValue::Array(vec![TestValue::Struct(
                    TestInstance::new(MOTION)
                        .value("actionController", TestValue::text(controller))
                        .value("animDatabase3P", TestValue::text(database)),
                )]),
            ),
        )
        .record(
            TestRecord::new(
                0x1054,
                ENTITY,
                "EntityClassDefinition.SyntheticWeapon",
                "libs/foundry/records/entities/scitem/weapons/synthetic_reload.xml",
            )
            .value(
                "Components",
                TestValue::Array(vec![TestValue::Struct(
                    TestInstance::new(ATTACHMENT).value(
                        "AttachDef",
                        TestValue::Struct(
                            TestInstance::new(ITEM)
                                .value("Type", TestValue::text("WeaponPersonal"))
                                .value("Tags", TestValue::text("matching_tag")),
                        ),
                    ),
                )]),
            ),
        )
        .record(
            TestRecord::new(
                0x1052,
                ENTITY,
                "EntityClassDefinition.SyntheticOtherItem",
                "libs/foundry/records/entities/scitem/weapons/synthetic_other.xml",
            )
            .value("Components", TestValue::Array(Vec::new())),
        )
        .build()
}

pub(super) fn documents() -> Vec<TestEntry> {
    [
        ("TestController.xml", r#"<ControllerDef><Fragments filename="Animations/SharedFragments.xml"/></ControllerDef>"#),
        ("TestRoot.adb", ROOT),
        ("Candidate.adb", CANDIDATE),
        ("WrongScope.adb", WRONG_SCOPE),
        ("SharedFragments.xml", r#"<TagDefinition version="2"><Tags><Tag name="Reload"/></Tags></TagDefinition>"#),
        ("OtherFragments.xml", r#"<TagDefinition version="2"><Tags><Tag name="Reload"/></Tags></TagDefinition>"#),
        ("SharedTags.xml", r#"<TagDefinition version="2"><Imports><Import filename="Animations/ImportedTags.xml"/></Imports></TagDefinition>"#),
        ("ImportedTags.xml", r#"<TagDefinition version="2"><Tags><Group name="Synthetic"><Tag name="matching_tag"/></Group></Tags></TagDefinition>"#),
    ].into_iter().map(|(path, content)| TestEntry::stored(format!("Data/Animations/{path}"), content.as_bytes())).collect()
}

const ROOT: &str = r#"<AnimDB FragDef="Animations/SharedFragments.xml" TagDef="Animations/SharedTags.xml"><SubADBs>
    <SubADB File="Animations/Candidate.adb"><FragmentID Name="Reload"/><FragmentID Name="NotPresent"/></SubADB>
    <SubADB File="Animations/WrongScope.adb"><FragmentID Name="Reload"/></SubADB>
    <SubADB File="Animations/Missing.adb"><FragmentID Name="Reload"/></SubADB>
    </SubADBs></AnimDB>"#;
pub(super) const CANDIDATE: &str = r#"<AnimDB FragDef="Animations/SharedFragments.xml" TagDef="Animations/SharedTags.xml">
    <SubADBs><SubADB File="Animations/TestRoot.adb"/></SubADBs>
    <FragmentList><Reload>
      <Fragment Tags="matching_tag" FragTags="" BlendOutDuration="0.2">
        <AnimLayer><Blend Duration="0.2" StartTime="0.3" ExitTime="0.4"/><Animation name="synthetic" speed="1.2" flags="4"/></AnimLayer>
        <ProcLayer><Procedural Procedure="synthetic"/></ProcLayer>
      </Fragment>
      <Fragment Tags="matching_tag" BlendOutDuration="0.5"><AnimLayer><Blend Duration="0.6"/><Animation name="second"/></AnimLayer></Fragment>
    </Reload></FragmentList></AnimDB>"#;
const WRONG_SCOPE: &str = r#"<AnimDB FragDef="Animations/OtherFragments.xml" TagDef="Animations/SharedTags.xml">
    <FragmentList><Reload><Fragment Tags="matching_tag" BlendOutDuration="4"/></Reload></FragmentList></AnimDB>"#;
