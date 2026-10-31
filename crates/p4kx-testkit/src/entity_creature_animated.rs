// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::entity_creature::skinned_creature;
use crate::{
    TestAnimation, TestEntity, TestEntry, TestInstance, TestSkeleton, TestStruct, TestTrack,
    TestType, TestValue,
};

/// A creature of [`CREATURE`](crate::CREATURE)'s skeleton with an animation database.
///
/// The database names the clips `wave`, `stray`, `lean` and `1d-bspace_walk`: `wave` moves the
/// bone `tip`, `stray` only a bone the skeleton lacks, `lean` is additive, and the blend space has
/// no file.
pub const CREATURE_ANIMATED: &str = "Test_Creature_Animated";

/// Like [`CREATURE_ANIMATED`], but the animation database it names is not in the Archive.
pub const CREATURE_ANIMATION_DATABASE_LOST: &str = "Test_Creature_AnimationDatabaseLost";

/// Like [`CREATURE_ANIMATED`], but its skeleton has no parameter file.
pub const CREATURE_ANIMATION_PARAMS_LOST: &str = "Test_Creature_AnimationParamsLost";

/// Like [`CREATURE_ANIMATED`], but its database nests one that is no database; the clip `wave`
/// is still named.
pub const CREATURE_ANIMATION_NESTED_LOST: &str = "Test_Creature_AnimationNestedLost";

const DATABASE: &str = "Animations/Mannequin/ADB/Creatures/Test/test.adb";
const SKELETON_FOLDER: &str = r"Data\Objects\Characters\Creatures\Test";
const CLIP_FOLDER: &str = r"Data\Animations\Characters\Creatures\Test\ai";

const ADB: &str = r#"<AnimDB>
  <FragmentList>
    <Idle>
      <Fragment><AnimLayer><Animation name="wave"/></AnimLayer></Fragment>
      <Fragment><AnimLayer><Animation name="1d-bspace_walk"/><Animation name="stray"/></AnimLayer></Fragment>
      <Fragment><AnimLayer><Animation name="lean"/></AnimLayer></Fragment>
    </Idle>
  </FragmentList>
</AnimDB>"#;

const NESTED_ADB: &str = r#"<AnimDB>
  <SubADBs><SubADB File="Animations/Mannequin/ADB/Creatures/Test/broken.adb"/></SubADBs>
  <FragmentList><Idle><Fragment><AnimLayer><Animation name="wave"/></AnimLayer></Fragment></Idle></FragmentList>
</AnimDB>"#;

const PARAMS: &str = r##"<Params>
  <AnimationList>
    <Animation name="#filepath" path="Animations/Characters/Creatures/Test"/>
    <Animation name="*" path="*/*.caf"/>
  </AnimationList>
</Params>"##;

/// Adds the struct of the component that names an animation database.
pub(crate) fn animated_structs() -> [TestStruct; 1] {
    [TestStruct::new("SAnimatedCharacterComponentParams")
        .parent("DataForgeComponentParams")
        .field("animDatabase3P", TestType::String)]
}

fn animated(id: u128, class: &str, skeleton: &str, database: &str) -> TestEntity {
    skinned_creature(
        id,
        class,
        &format!("Objects/Characters/Creatures/Test/{skeleton}"),
    )
    .component(TestValue::Struct(
        TestInstance::new("SAnimatedCharacterComponentParams")
            .value("animDatabase3P", TestValue::text(database)),
    ))
}

/// Returns the creatures [`CREATURE_ANIMATED`], [`CREATURE_ANIMATION_DATABASE_LOST`] and
/// [`CREATURE_ANIMATION_PARAMS_LOST`] and [`CREATURE_ANIMATION_NESTED_LOST`].
pub(crate) fn animated_entities() -> [TestEntity; 4] {
    [
        animated(0x20, CREATURE_ANIMATED, "creature_skel.chr", DATABASE),
        animated(
            0x21,
            CREATURE_ANIMATION_DATABASE_LOST,
            "creature_skel.chr",
            "Animations/Mannequin/ADB/Creatures/Test/absent.adb",
        ),
        animated(
            0x22,
            CREATURE_ANIMATION_PARAMS_LOST,
            "short_skel.chr",
            DATABASE,
        ),
        animated(
            0x23,
            CREATURE_ANIMATION_NESTED_LOST,
            "creature_skel.chr",
            "Animations/Mannequin/ADB/Creatures/Test/nested.adb",
        ),
    ]
}

/// Returns the Entries the animated creatures need besides their models: the parameter file of
/// `creature_skel.chr`, the animation databases and the three clips.
pub(crate) fn animation_entries() -> Vec<TestEntry> {
    const IDENTITY: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
    const TURNED: [f32; 4] = [
        0.0,
        0.0,
        std::f32::consts::FRAC_1_SQRT_2,
        std::f32::consts::FRAC_1_SQRT_2,
    ];
    let clip = |name: &str, controller: u32| {
        TestAnimation::new(name, 30)
            .track(TestTrack::new(controller).rotations(&[(0, IDENTITY), (30, TURNED)]))
    };
    let tip = TestSkeleton::controller_id(2);
    let caf = |animation: &TestAnimation| {
        TestEntry::stored(
            format!("{CLIP_FOLDER}\\{}.caf", animation.name()),
            animation.caf(),
        )
    };
    vec![
        TestEntry::stored(
            format!("{SKELETON_FOLDER}\\creature_skel.chrparams"),
            PARAMS.as_bytes().to_vec(),
        ),
        TestEntry::stored(
            r"Data\Animations\Mannequin\ADB\Creatures\Test\test.adb",
            ADB.as_bytes().to_vec(),
        ),
        caf(&clip("wave", tip)),
        caf(&clip("stray", 0x1234_5678)),
        caf(&clip("lean", tip).additive()),
        TestEntry::stored(
            r"Data\Animations\Mannequin\ADB\Creatures\Test\nested.adb",
            NESTED_ADB.as_bytes().to_vec(),
        ),
        TestEntry::stored(
            r"Data\Animations\Mannequin\ADB\Creatures\Test\broken.adb",
            b"not xml".to_vec(),
        ),
    ]
}
