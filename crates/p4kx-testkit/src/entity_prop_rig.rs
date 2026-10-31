// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::entity_creature::{bone, skeleton_file};
use crate::model_cube::cube_nodes;
use crate::{
    DataForgeBuilder, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry, TestModel, TestNode,
    TestPort, entity_schema,
};

/// A character whose root geometry is a human skeleton file (`.chr`) with the bones `pelvis`,
/// `root` and `tip` one metre apart along Z, wearing a skin of the bones `root` and `tip` whose
/// item ports hold props.
pub const PROP_RIG_HUMAN: &str = "Test_Prop_Human";

/// Like [`PROP_RIG_HUMAN`], but a creature: its skeleton file lies outside the human rigs.
pub const PROP_RIG_CREATURE: &str = "Test_Prop_Creature";

/// Like [`PROP_RIG_HUMAN`], but its root geometry is no character file.
pub const PROP_RIG_STATIC: &str = "Test_Prop_Static";

/// The item port of the skin of [`PROP_RIG_HUMAN`] that holds a rigid prop through a skin
/// attachment to the bone `pelvis`, a bone of the skeleton and none of the skin.
pub const PROP_ON_SKIN_PORT: &str = "Watch_ItemPort";

/// The item port that holds a rigid prop through a skin attachment to a bone no skeleton has.
pub const PROP_ON_LOST_PORT: &str = "Lost_ItemPort";

/// The item port that holds a skinned part through a skin attachment to the bone `pelvis`.
pub const SKINNED_ON_SKIN_PORT: &str = "Worn_ItemPort";

/// The item port that holds a rigid prop through a bone attachment to the bone `pelvis`.
pub const PROP_ON_BONE_PORT: &str = "Hand_ItemPort";

/// The item port that holds a rigid garment, an Entity of the item type `Char_Clothing_Legs`,
/// through a skin attachment to the bone `pelvis`.
pub const GARMENT_ON_SKIN_PORT: &str = "Garment_ItemPort";

/// The item port that holds a rigid prop through a skin attachment to `Slot_Node`, a node of the
/// skin and no bone of the skeleton.
pub const NODE_ON_SKIN_PORT: &str = "Slot_ItemPort";

/// The item port that holds a prop whose model has no meshes, through a skin attachment to the
/// bone `pelvis`.
pub const EMPTY_ON_SKIN_PORT: &str = "Empty_ItemPort";

/// Like [`PROP_RIG_HUMAN`], but its skeleton file is not in the Archive.
pub const PROP_RIG_NO_SKELETON: &str = "Test_Prop_NoSkeleton";

/// Like [`PROP_RIG_HUMAN`], but its skeleton file is no chunk file.
pub const PROP_RIG_BAD_SKELETON: &str = "Test_Prop_BadSkeleton";

const NODE_HELPER: &str = "Slot_Node";
const HUMAN_FOLDER: &str = "Objects/Characters/Human/Test";
const CREATURE_FOLDER: &str = "Objects/Characters/Creatures/Test";
const STATIC_FOLDER: &str = "Objects/Props/Test";

fn root(id: u128, class: &str, skeleton: &str) -> TestEntity {
    TestEntity::new(id, class, format!("entities/{class}.xml"))
        .geometry(skeleton, None)
        .port(TestPort::new("Body_ItemPort").skin())
        .loadout(TestLoadout::Manual(vec![
            TestLoadoutEntry::class("Body_ItemPort", "Test_Prop_Skin").loadout(
                TestLoadout::Manual(vec![
                    TestLoadoutEntry::class(PROP_ON_SKIN_PORT, "Test_Prop_Rigid"),
                    TestLoadoutEntry::class(PROP_ON_LOST_PORT, "Test_Prop_Rigid"),
                    TestLoadoutEntry::class(SKINNED_ON_SKIN_PORT, "Test_Prop_Worn"),
                    TestLoadoutEntry::class(PROP_ON_BONE_PORT, "Test_Prop_Rigid"),
                    TestLoadoutEntry::class(GARMENT_ON_SKIN_PORT, "Test_Prop_Garment"),
                    TestLoadoutEntry::class(NODE_ON_SKIN_PORT, "Test_Prop_Rigid"),
                    TestLoadoutEntry::class(EMPTY_ON_SKIN_PORT, "Test_Prop_Empty"),
                ]),
            ),
        ]))
}

/// The item ports of the skin: the name, the helper and whether the attachment is a skin.
const SKIN_PORTS: [(&str, &str, bool); 7] = [
    (PROP_ON_SKIN_PORT, "pelvis", true),
    (PROP_ON_LOST_PORT, "nowhere", true),
    (SKINNED_ON_SKIN_PORT, "pelvis", true),
    (PROP_ON_BONE_PORT, "pelvis", false),
    (GARMENT_ON_SKIN_PORT, "pelvis", true),
    (NODE_ON_SKIN_PORT, NODE_HELPER, true),
    (EMPTY_ON_SKIN_PORT, "pelvis", true),
];

/// Returns the skin entity that holds the item ports of [`SKIN_PORTS`].
fn skin_entity() -> TestEntity {
    SKIN_PORTS.iter().fold(
        TestEntity::new(0x10, "Test_Prop_Skin", "entities/Test_Prop_Skin.xml")
            .geometry(format!("{HUMAN_FOLDER}/body.skin"), None),
        |entity, &(name, helper, is_skin)| {
            let port = TestPort::new(name).helper(helper, [0.0; 3], [0.0; 3]);
            entity.port(if is_skin { port.skin() } else { port })
        },
    )
}

fn database() -> DataForgeBuilder {
    let skin = skin_entity();
    let garment = TestEntity::new(0x13, "Test_Prop_Garment", "entities/Test_Prop_Garment.xml")
        .attach_type("Char_Clothing_Legs")
        .geometry("Objects/Test/prop.cgf", None);
    let rigid = TestEntity::new(0x11, "Test_Prop_Rigid", "entities/Test_Prop_Rigid.xml")
        .geometry("Objects/Test/prop.cgf", None);
    let worn = TestEntity::new(0x12, "Test_Prop_Worn", "entities/Test_Prop_Worn.xml")
        .geometry(format!("{HUMAN_FOLDER}/body.skin"), None);
    let empty = TestEntity::new(0x14, "Test_Prop_Empty", "entities/Test_Prop_Empty.xml")
        .geometry("Objects/Test/empty.cgf", None);
    [
        root(
            0x4,
            PROP_RIG_NO_SKELETON,
            &format!("{HUMAN_FOLDER}/absent_skeleton.chr"),
        ),
        root(
            0x5,
            PROP_RIG_BAD_SKELETON,
            &format!("{HUMAN_FOLDER}/bad_skeleton.chr"),
        ),
        empty,
        root(0x1, PROP_RIG_HUMAN, &format!("{HUMAN_FOLDER}/skeleton.chr")),
        root(
            0x2,
            PROP_RIG_CREATURE,
            &format!("{CREATURE_FOLDER}/skeleton.chr"),
        ),
        root(
            0x3,
            PROP_RIG_STATIC,
            &format!("{STATIC_FOLDER}/skeleton.chr"),
        ),
        skin,
        rigid,
        worn,
        garment,
    ]
    .iter()
    .fold(entity_schema(DataForgeBuilder::new()), |builder, entity| {
        builder.record(entity.record())
    })
}

/// Returns the Entries of a synthetic Archive with the characters [`PROP_RIG_HUMAN`],
/// [`PROP_RIG_CREATURE`] and the prop [`PROP_RIG_STATIC`].
///
/// Each has the skeleton `pelvis`, `root`, `tip` and a two-bone skin on the skin port
/// `Body_ItemPort`; the item ports of that skin are [`PROP_ON_SKIN_PORT`], [`PROP_ON_LOST_PORT`],
/// [`SKINNED_ON_SKIN_PORT`], [`PROP_ON_BONE_PORT`] and [`GARMENT_ON_SKIN_PORT`].
#[must_use]
pub fn prop_rig_entries() -> Vec<TestEntry> {
    let skin = TestModel::two_bone_skin().with_nodes(skin_nodes());
    let prop = TestModel::cube().scaled(0.25);
    let empty = TestModel::cube().helpers_only();
    let bones = || {
        skeleton_file(vec![
            bone("pelvis", None),
            bone("root", Some(0)),
            bone("tip", Some(1)),
        ])
    };
    let stored = |path: &str, bytes: Vec<u8>| TestEntry::stored(path.replace('/', "\\"), bytes);
    vec![
        TestEntry::zstd("Data\\Game2.dcb", database().build()),
        stored(&format!("Data/{HUMAN_FOLDER}/skeleton.chr"), bones()),
        stored(&format!("Data/{CREATURE_FOLDER}/skeleton.chr"), bones()),
        stored(&format!("Data/{STATIC_FOLDER}/skeleton.chr"), bones()),
        stored(&format!("Data/{HUMAN_FOLDER}/body.skin"), skin.cgf()),
        stored(&format!("Data/{HUMAN_FOLDER}/body.skinm"), skin.cgfm()),
        stored(
            &format!("Data/{HUMAN_FOLDER}/bad_skeleton.chr"),
            b"not a chunk file".to_vec(),
        ),
        stored("Data/Objects/Test/prop.cgf", prop.cgf()),
        stored("Data/Objects/Test/prop.cgfm", prop.cgfm()),
        stored("Data/Objects/Test/empty.cgf", empty.cgf()),
    ]
}

/// Returns the nodes of the skin: the model's own and the helper node `Slot_Node`.
fn skin_nodes() -> Vec<TestNode> {
    let mut nodes = cube_nodes();
    nodes.push(TestNode {
        name: NODE_HELPER.to_owned(),
        parent: Some(0),
        local: [
            [1.0, 0.0, 0.0, 5.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
        kind: 3,
    });
    nodes
}
