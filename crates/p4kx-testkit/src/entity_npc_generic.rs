// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::entity_armour::{armour_database, armour_files};
use crate::{TestEntity, TestEntry, TestLoadout, TestLoadoutEntry, TestPort};

/// The class name of the NPC of [`npc_generic_entries`].
pub const NPC_GENERIC_CLASS: &str = "TEST_NpcGeneric";
/// The class name of the generic head of [`npc_generic_entries`], which names no body and no model.
pub const NPC_GENERIC_HEAD_CLASS: &str = "TEST_NpcGenericHead";
/// The class name of the `Char_Body` Entity of [`npc_generic_entries`], sub-type `Female`.
pub const NPC_GENERIC_BODY_CLASS: &str = "TEST_NpcGenericBody";

const SKIN: &str = "Objects/Characters/Human/female_v2/armor/test/f_suit.skin";

fn worn(guid: u128, class: &str, item_type: &str) -> TestEntity {
    TestEntity::new(guid, class, format!("entities/scitem/{class}.xml")).attach_type(item_type)
}

fn entities() -> Vec<TestEntity> {
    let body = worn(0x340, NPC_GENERIC_BODY_CLASS, "Char_Body").sub_type("Female");
    let head = worn(0x341, NPC_GENERIC_HEAD_CLASS, "Char_Head")
        .sub_type("UNDEFINED")
        .port(TestPort::new("Eyelashes_ItemPort"));
    let lashes = worn(0x342, "TEST_NpcGenericLashes", "Char_Head_Eyelash")
        .geometry_variant("Female", SKIN, None);
    let shirt = worn(0x343, "TEST_NpcGenericShirt", "Char_Clothing_Torso_0")
        .geometry_variant("Female", SKIN, None);
    let loadout = |port: &str, class: &str, nested: Vec<TestLoadoutEntry>| {
        TestLoadoutEntry::class(port, class).loadout(TestLoadout::Manual(nested))
    };
    let npc = TestEntity::new(
        0x344,
        NPC_GENERIC_CLASS,
        "entities/scitem/test_npc_generic.xml",
    )
    .loadout(TestLoadout::Manual(vec![loadout(
        "Body_ItemPort",
        NPC_GENERIC_BODY_CLASS,
        vec![
            loadout(
                "Head_ItemPort",
                NPC_GENERIC_HEAD_CLASS,
                vec![TestLoadoutEntry::class(
                    "Eyelashes_ItemPort",
                    "TEST_NpcGenericLashes",
                )],
            ),
            TestLoadoutEntry::class("Clothing_Torso_0", "TEST_NpcGenericShirt"),
        ],
    )]));
    vec![body, head, lashes, shirt, npc]
}

/// Returns the Entries of a synthetic Archive with the generic-head NPC [`NPC_GENERIC_CLASS`].
///
/// Its default Loadout puts a female `Char_Body` Entity on `Body_ItemPort`, with the generic head
/// [`NPC_GENERIC_HEAD_CLASS`] (`UNDEFINED`, no model) carrying one eyelash piece and a shirt with a
/// `Female` sub-geometry. The bodies and the skins of [`armour_entries`](crate::armour_entries) are
/// included.
#[must_use]
pub fn npc_generic_entries() -> Vec<TestEntry> {
    let builder = entities()
        .iter()
        .fold(armour_database(), |builder, entity| {
            builder.record(entity.record())
        });
    let mut entries = vec![TestEntry::zstd("Data\\Game2.dcb", builder.build())];
    entries.extend(armour_files());
    entries
}
