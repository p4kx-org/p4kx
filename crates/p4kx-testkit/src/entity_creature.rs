// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::entity_creature_animated::{animated_entities, animated_structs, animation_entries};
use crate::{
    DataForgeBuilder, TestBone, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry, TestModel,
    TestNode, TestPort, TestRecord, TestSkeleton, assortment_record, entity_schema,
    model_cube::cube_nodes,
};

/// A creature whose root geometry is a skeleton file (`.chr`) with the bones `pelvis`, `root`
/// and `tip`, wearing a skin of the bones `root` and `tip` on a skin port.
pub const CREATURE: &str = "Test_Creature";

/// Like [`CREATURE`], but its skeleton lacks the bone `tip` that its skin is weighted to.
pub const CREATURE_SHORT_SKELETON: &str = "Test_Creature_Short";

/// A creature whose root geometry is a character definition that names the skeleton file of
/// [`CREATURE`], wearing the same skin.
pub const CREATURE_DEFINITION: &str = "Test_Creature_Definition";

/// Like [`CREATURE_DEFINITION`], but the skeleton file it names is not in the Archive.
pub const CREATURE_NO_SKELETON: &str = "Test_Creature_NoSkeleton";

/// Like [`CREATURE`], with a rigid horn on a bone port at the bone `tip` beside the skinned body.
pub const CREATURE_HORNED: &str = "Test_Creature_Horned";

/// Like [`CREATURE`], with a rigid collar on an item port of the skin, at the helper `pelvis`, a
/// bone of the creature's skeleton and no node or bone of the skin's own model.
pub const CREATURE_COLLARED: &str = "Test_Creature_Collared";

/// Like [`CREATURE_COLLARED`], but the helper of the collar port is no bone of the skeleton.
pub const CREATURE_COLLAR_LOST: &str = "Test_Creature_CollarLost";

/// Like [`CREATURE_COLLARED`], but the skin has a node named like the collar port as well; the bone
/// its helper names still decides.
pub const CREATURE_COLLAR_NODED: &str = "Test_Creature_CollarNoded";

/// Like [`CREATURE`], but its body port is unset in the Record and its default Loadout is
/// generated at random from an assortment whose first body alternative is the skin.
pub const CREATURE_ASSORTED: &str = "Test_Creature_Assorted";

/// Like [`CREATURE`], but its body port is unset and its Loadout is an XML file no one reads.
pub const CREATURE_LOADOUT_LOST: &str = "Test_Creature_LoadoutLost";

pub use crate::entity_creature_animated::{
    CREATURE_ANIMATED, CREATURE_ANIMATION_DATABASE_LOST, CREATURE_ANIMATION_NESTED_LOST,
    CREATURE_ANIMATION_PARAMS_LOST,
};

/// A rigged Entity outside the characters folder, a prop with a skeleton file and the same skin.
pub const RIGGED_PROP: &str = "Test_Rigged_Prop";

const FOLDER: &str = "Objects/Characters/Creatures/Test";
const ASSORTMENT: u128 = 0x100;

fn definition(skeleton: &str) -> String {
    format!(
        r#"<CharacterDefinition>
  <Model File="{FOLDER}/{skeleton}"/>
  <AttachmentList>
    <Attachment Type="CA_SKIN" AName="Body" Binding="{FOLDER}/creature.skin"/>
  </AttachmentList>
</CharacterDefinition>"#
    )
}

pub(crate) fn bone(name: &str, parent: Option<u16>) -> TestBone {
    TestBone {
        name: name.to_owned(),
        parent,
        rotation: [0.0, 0.0, 0.0, 1.0],
        translation: [0.0, 0.0, 1.0],
    }
}

pub(crate) fn skeleton_file(bones: Vec<TestBone>) -> Vec<u8> {
    TestModel::cube()
        .helpers_only()
        .with_skeleton(&TestSkeleton::new(bones))
        .cgf()
}

/// Returns a creature Entity of the root geometry `geometry` that wears the skin
/// `Test_Creature_Skin` on its skin port `Body_ItemPort`.
pub(crate) fn skinned_creature(id: u128, class: &str, geometry: &str) -> TestEntity {
    TestEntity::new(id, class, format!("entities/{class}.xml"))
        .geometry(geometry, None)
        .port(TestPort::new("Body_ItemPort").skin())
        .loadout(TestLoadout::Manual(vec![TestLoadoutEntry::class(
            "Body_ItemPort",
            "Test_Creature_Skin",
        )]))
}

fn database() -> DataForgeBuilder {
    let skin = TestEntity::new(
        0x10,
        "Test_Creature_Skin",
        "entities/Test_Creature_Skin.xml",
    )
    .geometry(format!("{FOLDER}/creature.skin"), None);
    let horned = TestEntity::new(0x5, CREATURE_HORNED, "entities/Test_Creature_Horned.xml")
        .geometry(format!("{FOLDER}/creature_skel.chr"), None)
        .port(TestPort::new("Body_ItemPort").skin())
        .port(TestPort::new("Horn_ItemPort").helper("tip", [0.0; 3], [0.0; 3]))
        .loadout(TestLoadout::Manual(vec![
            TestLoadoutEntry::class("Body_ItemPort", "Test_Creature_Skin"),
            TestLoadoutEntry::class("Horn_ItemPort", "Test_Creature_Horn"),
        ]));
    let horn = TestEntity::new(
        0x11,
        "Test_Creature_Horn",
        "entities/Test_Creature_Horn.xml",
    )
    .geometry("Objects/Test/horn.cgf", None);
    [
        skinned_creature(0x1, CREATURE, &format!("{FOLDER}/creature_skel.chr")),
        skinned_creature(
            0x2,
            CREATURE_SHORT_SKELETON,
            &format!("{FOLDER}/short_skel.chr"),
        ),
        skinned_creature(
            0x3,
            CREATURE_DEFINITION,
            &format!("{FOLDER}/definition.cdf"),
        ),
        skinned_creature(
            0x4,
            CREATURE_NO_SKELETON,
            &format!("{FOLDER}/no_skeleton.cdf"),
        ),
        skinned_creature(0x6, RIGGED_PROP, "Objects/Props/Test/prop_skel.chr"),
        horned,
        horn,
        skin,
    ]
    .into_iter()
    .chain(collar_entities())
    .chain(unset_body_entities())
    .chain(animated_entities())
    .fold(
        animated_structs().into_iter().fold(
            entity_schema(DataForgeBuilder::new()),
            DataForgeBuilder::structure,
        ),
        |builder, entity| builder.record(entity.record()),
    )
    .record(creature_assortment())
}

/// Returns the assortment of [`CREATURE_ASSORTED`]: one group whose first alternative is the skin
/// and whose second is the horn.
fn creature_assortment() -> TestRecord {
    let body = |class| TestLoadout::Manual(vec![TestLoadoutEntry::class("Body_ItemPort", class)]);
    assortment_record(
        ASSORTMENT,
        "test_creature",
        &[(
            "Body",
            vec![body("Test_Creature_Skin"), body("Test_Creature_Horn")],
        )],
    )
}

/// Returns the creatures [`CREATURE_ASSORTED`] and [`CREATURE_LOADOUT_LOST`], whose body port the
/// Record leaves unset.
fn unset_body_entities() -> [TestEntity; 2] {
    let creature = |id, class: &str, loadout| {
        TestEntity::new(id, class, format!("entities/{class}.xml"))
            .geometry(format!("{FOLDER}/creature_skel.chr"), None)
            .port(TestPort::new("Body_ItemPort").skin())
            .loadout(loadout)
    };
    [
        creature(
            0xa,
            CREATURE_ASSORTED,
            TestLoadout::RandomGenerated(vec![ASSORTMENT]),
        ),
        creature(
            0xb,
            CREATURE_LOADOUT_LOST,
            TestLoadout::Xml("Scripts/Loadouts/Creatures/lost.xml".to_owned()),
        ),
    ]
}

/// Returns the creatures [`CREATURE_COLLARED`] and [`CREATURE_COLLAR_LOST`] with the skins that
/// hold the collar port, whose helper is the bone `pelvis` and the name `nowhere`.
fn collar_entities() -> [TestEntity; 6] {
    let creature = |id, class: &str, skin: &str| {
        TestEntity::new(id, class, format!("entities/{class}.xml"))
            .geometry(format!("{FOLDER}/creature_skel.chr"), None)
            .port(TestPort::new("Body_ItemPort").skin())
            .loadout(TestLoadout::Manual(vec![
                TestLoadoutEntry::class("Body_ItemPort", skin).loadout(TestLoadout::Manual(vec![
                    TestLoadoutEntry::class("Collar_ItemPort", "Test_Creature_Horn"),
                ])),
            ]))
    };
    let skin = |id, class: &str, file: &str, helper: &str| {
        TestEntity::new(id, class, format!("entities/{class}.xml"))
            .geometry(format!("{FOLDER}/{file}"), None)
            .port(TestPort::new("Collar_ItemPort").helper(helper, [0.0; 3], [0.0; 3]))
    };
    [
        creature(0x7, CREATURE_COLLARED, "Test_Creature_Collar_Skin"),
        creature(0x8, CREATURE_COLLAR_LOST, "Test_Creature_Lost_Skin"),
        creature(0x9, CREATURE_COLLAR_NODED, "Test_Creature_Noded_Skin"),
        skin(0x12, "Test_Creature_Collar_Skin", "creature.skin", "pelvis"),
        skin(0x13, "Test_Creature_Lost_Skin", "creature.skin", "nowhere"),
        skin(0x14, "Test_Creature_Noded_Skin", "noded.skin", "pelvis"),
    ]
}

/// Returns the two-bone skin with a helper node named like the collar port, away from the origin.
fn noded_skin() -> TestModel {
    let mut nodes = cube_nodes();
    nodes.push(TestNode {
        name: "Collar_ItemPort".to_owned(),
        parent: Some(0),
        local: [
            [1.0, 0.0, 0.0, 5.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
        kind: 3,
    });
    TestModel::two_bone_skin().with_nodes(nodes)
}

/// Returns the Entries of a synthetic Archive that holds creatures with a skeleton and a skin.
///
/// The creatures [`CREATURE`], [`CREATURE_SHORT_SKELETON`], [`CREATURE_DEFINITION`],
/// [`CREATURE_NO_SKELETON`] and [`CREATURE_HORNED`], and the prop [`RIGGED_PROP`], each hold the
/// same two-bone skin on the skin port `Body_ItemPort`. The creatures [`CREATURE_ANIMATED`],
/// [`CREATURE_ANIMATION_DATABASE_LOST`] and [`CREATURE_ANIMATION_PARAMS_LOST`] name an animation
/// database as well.
#[must_use]
pub fn creature_entries() -> Vec<TestEntry> {
    let skin = TestModel::two_bone_skin();
    let horn = TestModel::cube().scaled(0.25);
    let entry = |name: &str, bytes: Vec<u8>| {
        TestEntry::stored(
            format!("Data\\{}", format!("{FOLDER}/{name}").replace('/', "\\")),
            bytes,
        )
    };
    vec![
        TestEntry::zstd("Data\\Game2.dcb", database().build()),
        entry(
            "creature_skel.chr",
            skeleton_file(vec![
                bone("pelvis", None),
                bone("root", Some(0)),
                bone("tip", Some(1)),
            ]),
        ),
        entry(
            "short_skel.chr",
            skeleton_file(vec![bone("pelvis", None), bone("root", Some(0))]),
        ),
        TestEntry::stored(
            r"Data\Objects\Props\Test\prop_skel.chr",
            skeleton_file(vec![
                bone("pelvis", None),
                bone("root", Some(0)),
                bone("tip", Some(1)),
            ]),
        ),
        TestEntry::stored(r"Data\Objects\Test\horn.cgf", horn.cgf()),
        TestEntry::stored(r"Data\Objects\Test\horn.cgfm", horn.cgfm()),
        entry("creature.skin", skin.cgf()),
        entry("creature.skinm", skin.cgfm()),
        entry("noded.skin", noded_skin().cgf()),
        entry("noded.skinm", noded_skin().cgfm()),
        entry(
            "definition.cdf",
            definition("creature_skel.chr").into_bytes(),
        ),
        entry(
            "no_skeleton.cdf",
            definition("absent_skel.chr").into_bytes(),
        ),
    ]
    .into_iter()
    .chain(animation_entries())
    .collect()
}
