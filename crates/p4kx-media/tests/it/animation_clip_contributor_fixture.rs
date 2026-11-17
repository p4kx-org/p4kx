// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestAnimation, TestEntity, TestEntry, TestInstance,
    TestLoadout, TestLoadoutEntry, TestModel, TestPort, TestSkeleton, TestStruct, TestTrack,
    TestType, TestValue, entity_schema, test_key,
};

pub(super) const ENTITY: &str = "FixtureClipOwner";
const PART: &str = "FixtureClipAttachment";
const COMPONENT: &str = "SAnimatedCharacterComponentParams";
const FIELD: &str = "animDatabase3P";
const ROOT_CHR: &str = "Data/Objects/clip-owner.chr";
const PART_CHR: &str = "Data/Objects/clip-part.chr";
pub(super) const ROOT_PARAMS: &str = "Data/Objects/clip-owner.chrparams";
pub(super) const PART_PARAMS: &str = "Data/Objects/clip-part.chrparams";
pub(super) const INCLUDED_PARAMS: &str = "Data/Objects/clip-included.chrparams";
pub(super) const ROOT_ADB: &str = "Data/Animations/Contributor/root.adb";
pub(super) const CHILD_ADB: &str = "Data/Animations/Contributor/child.adb";
pub(super) const PART_ADB: &str = "Data/Animations/Contributor/part.adb";
pub(super) const ROOT_CAF: &str = "Data/Animations/Contributor/root.caf";
pub(super) const ROOT_ONLY_CAF: &str = "Data/Animations/Contributor/root_only.caf";
pub(super) const PART_CAF: &str = "Data/Animations/Contributor/part.caf";
pub(super) const PART_ONLY_CAF: &str = "Data/Animations/Contributor/part_only.caf";
pub(super) const INCLUDED_CAF: &str = "Data/Animations/Contributor/included.caf";
pub(super) const INCLUDE_SHARED_CAF: &str = "Data/Animations/Contributor/include_shared.caf";
pub(super) const EXACT_WILDCARD_CAF: &str = "Data/Animations/Contributor/Parts/load_shared.caf";
pub(super) const FOLDED_WILDCARD_CAF: &str = "Data/Animations/Contributor/Parts/load_SHARED.caf";

const ROOT_XML: &str = concat!(
    r#"<AnimDB><FragmentList><Idle><Fragment Tags="Root" OptionWeight="0.25"><AnimLayer>"#,
    r#"<Animation name="shared"/><Animation name="shared"/><Animation name="PartOnly"/>"#,
    "</AnimLayer></Fragment></Idle></FragmentList><SubADBs>",
    r#"<SubADB File="Animations/Contributor/child.adb"/>"#,
    r#"<SubADB File="DATA\ANIMATIONS\Contributor\CHILD.adb"/>"#,
    "</SubADBs></AnimDB>",
);
const CHILD_XML: &str = concat!(
    "<AnimDB><FragmentList><Idle><Fragment><AnimLayer>",
    r#"<Animation name="includeOnly"/><Animation name="SHARED"/><Animation name=""/>"#,
    "</AnimLayer></Fragment></Idle></FragmentList>",
    r#"<SubADBs><SubADB File="Animations/Contributor/root.adb"/></SubADBs></AnimDB>"#,
);
const PART_XML: &str = concat!(
    r#"<AnimDB><FragmentList><Idle><Fragment Tags="Attachment" OptionWeight="0.75"><AnimLayer>"#,
    r#"<Animation name="shared"/><Animation name="RootOnly"/><Animation name="PartOnly"/>"#,
    "</AnimLayer></Fragment></Idle></FragmentList></AnimDB>",
);
const ROOT_PARAMETER_XML: &str = concat!(
    "<Params><AnimationList>",
    r#"<Animation Name="shared" Path="Animations/Contributor/root.caf"/>"#,
    r#"<Animation Name="RootOnly" Path="Animations/Contributor/root_only.caf"/>"#,
    r#"<Animation Name="$Include" Path="Data/Objects/clip-included.chrparams"/>"#,
    "</AnimationList></Params>",
);
const CHILD_PARAMETER_XML: &str = concat!(
    "<Params><AnimationList>",
    r#"<Animation Name="includeOnly" Path="Animations/Contributor/included.caf"/>"#,
    r#"<Animation Name="shared" Path="Animations/Contributor/include_shared.caf"/>"#,
    "</AnimationList></Params>",
);
const PART_PARAMETER_XML: &str = concat!(
    "<Params><AnimationList>",
    r#"<Animation Name="shared" Path="Animations/Contributor/part.caf"/>"#,
    r#"<Animation Name="PartOnly" Path="Animations/Contributor/part_only.caf"/>"#,
    r##"<Animation Name="#FilePath" Path="Animations/Contributor/Parts"/>"##,
    r#"<Animation Name="*" Path="load_*.caf"/>"#,
    "</AnimationList></Params>",
);

pub(super) fn open(folder: &Path) -> Result<Archive, Box<dyn Error>> {
    let path = folder.join("contributors.p4k");
    let mut archive = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", entities()))
        .entry(TestEntry::stored(ROOT_CHR, rig()))
        .entry(TestEntry::stored(PART_CHR, rig()))
        .entry(TestEntry::stored(
            ROOT_PARAMS,
            ROOT_PARAMETER_XML.as_bytes(),
        ))
        .entry(TestEntry::stored(
            PART_PARAMS,
            PART_PARAMETER_XML.as_bytes(),
        ))
        .entry(TestEntry::stored(INCLUDED_PARAMS, CHILD_PARAMETER_XML.as_bytes()).encrypted())
        .entry(TestEntry::stored(ROOT_ADB, ROOT_XML.as_bytes()))
        .entry(TestEntry::stored(CHILD_ADB, CHILD_XML.as_bytes()).encrypted())
        .entry(TestEntry::stored(PART_ADB, PART_XML.as_bytes()));
    for target in [
        ROOT_CAF,
        ROOT_ONLY_CAF,
        PART_CAF,
        PART_ONLY_CAF,
        INCLUDED_CAF,
        INCLUDE_SHARED_CAF,
        EXACT_WILDCARD_CAF,
        FOLDED_WILDCARD_CAF,
    ] {
        archive = archive.entry(TestEntry::stored(target, clip(target)));
    }
    archive.write_to(&path)?;
    Ok(Archive::open_with_key(path, Some(test_key()))?)
}

fn rig() -> Vec<u8> {
    TestModel::cube()
        .with_skeleton(&TestSkeleton::two_bones())
        .cgf()
}

fn clip(path: &str) -> Vec<u8> {
    let (rate, end_frame) = match path {
        ROOT_CAF => (24, 12),
        PART_CAF => (60, 48),
        _ => (30, 30),
    };
    TestAnimation::new("shared", rate)
        .track(
            TestTrack::new(TestSkeleton::controller_id(0))
                .positions(&[(0, [0.0; 3]), (end_frame, [1.0, 0.0, 0.0])]),
        )
        .caf()
}

fn entities() -> Vec<u8> {
    let root = TestEntity::new(1, ENTITY, "entities/clip-owner.xml")
        .geometry("Objects/clip-owner.chr", None)
        .loadout(TestLoadout::Manual(vec![
            TestLoadoutEntry::class("left", PART),
            TestLoadoutEntry::class("right", PART),
        ]))
        .port(TestPort::new("left"))
        .port(TestPort::new("right"))
        .component(selector("Animations/Contributor/root.adb"));
    let attachment = TestEntity::new(2, PART, "entities/clip-part.xml")
        .geometry("Objects/clip-part.chr", None)
        .component(selector("Animations/Contributor/part.adb"));
    entity_schema(DataForgeBuilder::new())
        .structure(
            TestStruct::new(COMPONENT)
                .parent("DataForgeComponentParams")
                .field(FIELD, TestType::String),
        )
        .record(root.record())
        .record(attachment.record())
        .build()
}

fn selector(path: &str) -> TestValue {
    TestValue::Struct(TestInstance::new(COMPONENT).value(FIELD, TestValue::text(path)))
}
