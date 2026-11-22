// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

const ANIMATION: &str = "WeaponProceduralAnimation";
const SETUP: &str = "WeaponProceduralClipsSetUp";
const CLIP: &str = "WeaponProceduralClip";
const STANCE: &str = "ActorStateFilterByStanceState";
pub(super) const ANIMATION_GUID: u128 = 0x9b21;
const CLIP_GUID: u128 = 0x9b22;

pub(super) const ANIMATION_INDEX: u32 = 0;
pub(super) const CLIP_INDEX: u32 = 1;
pub(super) const ANIMATION_ALIAS_INDEX: u32 = 2;

pub(super) fn database() -> DataForgeBuilder {
    let setup = TestInstance::new(SETUP)
        .value("filterName", TestValue::text("FixtureAim"))
        .value("filterByStanceState", TestValue::text("FixtureStand"))
        .value(
            "weaponProceduralClips",
            TestValue::Array(vec![TestValue::Reference(CLIP_GUID)]),
        )
        .value("fixtureExtra", TestValue::Bool(true));
    DataForgeBuilder::new()
        .version(8)
        .enumeration(STANCE, ["FixtureStand"])
        .structure(TestStruct::new(ANIMATION).array(
            "weaponProceduralClipsSetUp",
            TestType::Class(SETUP.to_owned()),
        ))
        .structure(
            TestStruct::new(SETUP)
                .field("filterName", TestType::String)
                .field("filterByStanceState", TestType::Enum(STANCE.to_owned()))
                .array(
                    "weaponProceduralClips",
                    TestType::ReferenceTo(CLIP.to_owned()),
                )
                .field("fixtureExtra", TestType::Bool),
        )
        .structure(TestStruct::new(CLIP).field("fixtureScale", TestType::Single))
        .record(animation("Primary").value(
            "weaponProceduralClipsSetUp",
            TestValue::Array(vec![
                TestValue::Struct(setup.clone()),
                TestValue::Struct(setup),
            ]),
        ))
        .record(
            TestRecord::new(
                CLIP_GUID,
                CLIP,
                "WeaponProceduralClip.Fixture",
                "libs/foundry/records/fixture/procedural_clip.xml",
            )
            .value("fixtureScale", TestValue::Single(0.5)),
        )
        .record(animation("Alias"))
}

fn animation(name: &str) -> TestRecord {
    TestRecord::new(
        ANIMATION_GUID,
        ANIMATION,
        format!("{ANIMATION}.{name}"),
        format!("libs/foundry/records/fixture/procedural_{name}.xml"),
    )
}
