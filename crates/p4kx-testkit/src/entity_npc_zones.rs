// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::entity_armour::MALE_ARMOUR;
use crate::entity_clothing::TestZoneChunk;
use crate::entity_zones::{zone_entries_with, zone_jacket, zone_shirt, zone_shirt_hiding};
use crate::{TestEntity, TestEntry, TestLoadout, TestLoadoutEntry, ZONE_JACKET, ZONE_SHIRT};

/// The class name of the character of [`npc_zone_entries`].
pub const NPC_ZONE_CLASS: &str = "TEST_ZoneNpc";

const ZONE_POUCH: &str = "TEST_ZonePouch";
const ZONE_GUN: &str = "TEST_ZoneGun";
const MALE_SKIN: &str = "Objects/Characters/Human/male_v7/body/m_body.skin";
const FEMALE_SKIN: &str = "Objects/Characters/Human/female_v2/body/f_body.skin";

/// How [`npc_zone_entries`] builds the character.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NpcZoneOptions {
    /// Makes the character female: its head is `Female` and its body the female skin.
    pub female: bool,
    /// Leaves the head out; the shirt then names the male suit skin as its main geometry, so that
    /// it has a model without a body to choose by.
    pub headless: bool,
    /// Gives the head this sub-type instead of the one of the body, e.g. `UNDEFINED` as the
    /// generic head of most NPCs has; the body Entity then carries the sub-type of its body.
    pub head_sub_type: Option<&'static str>,
    /// Gives the body Entity a copy of the body skin under this path below `Data` instead of the
    /// default skin file.
    pub body_skin: Option<&'static str>,
    /// Gives the body Entity another attach type than `Char_Body`.
    pub body_type: Option<&'static str>,
    /// Adds a second `Char_Body` Entity with the default skin on `Spare_ItemPort`.
    pub second_body: bool,
}

/// Returns the Entries of [`zone_entries`] with a character whose body Entity names the zoned
/// body skin and holds [`ZONE_SHIRT`], which covers `chunks`.
///
/// [`NPC_ZONE_CLASS`] has the Loadout `Body_ItemPort` holding `TEST_ZoneNpcBody` (`Char_Body`,
/// geometry the body skin of `options`), whose Loadout puts a head (`Char_Head`, `Male` or
/// `Female`) on `Head_ItemPort` unless `options.headless`, and the shirt on `Clothing_Torso_0`.
#[must_use]
pub fn npc_zone_entries(
    bodies: [&[[f32; 2]]; 2],
    chunks: &[TestZoneChunk],
    options: NpcZoneOptions,
) -> Vec<TestEntry> {
    npc_entries(bodies, zone_shirt(chunks), None, &[], options)
}

/// Something more [`npc_jacketed_entries`] puts in the tree of the character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcExtra {
    /// A pouch (`Char_Clothing_Backpack`) on `Pouch_ItemPort` of the shirt.
    Pouch,
    /// A class that does not exist on the port of this name below the body.
    Missing(&'static str),
    /// A weapon (`WeaponPersonal`) on the port of this name below the body.
    Weapon(&'static str),
    /// A second character: a body on `Second_ItemPort` of the root that wears the shirt on
    /// `Clothing_Torso_0`.
    SecondWearer,
}

/// Returns the Entries of [`npc_zone_entries`] with [`ZONE_JACKET`] worn at `Clothing_Torso_1`
/// too, and what `extras` add.
///
/// The shirt covers `shirt.0` and hides the items at the ports `shirt.1`; the jacket covers
/// `jacket.0` and hides the items at the ports `jacket.1`.
#[must_use]
pub fn npc_jacketed_entries(
    bodies: [&[[f32; 2]]; 2],
    shirt: (&[TestZoneChunk], &[&str]),
    jacket: (&[TestZoneChunk], &[&str]),
    extras: &[NpcExtra],
) -> Vec<TestEntry> {
    npc_entries(
        bodies,
        zone_shirt_hiding(shirt.0, shirt.1),
        Some(zone_jacket(jacket.0, jacket.1)),
        extras,
        NpcZoneOptions::default(),
    )
}

/// Returns the shirt as the Loadout of a body holds it, with the pouch on its port if asked.
fn shirt_entry(with_pouch: bool) -> TestLoadoutEntry {
    let entry = TestLoadoutEntry::class("Clothing_Torso_0", ZONE_SHIRT);
    if with_pouch {
        entry.loadout(TestLoadout::Manual(vec![TestLoadoutEntry::class(
            "Pouch_ItemPort",
            ZONE_POUCH,
        )]))
    } else {
        entry
    }
}

/// Returns what `extra` puts below the body, if it puts something there.
fn extra_entry(extra: &NpcExtra) -> Option<TestLoadoutEntry> {
    match extra {
        NpcExtra::Missing(port) => Some(TestLoadoutEntry::class(*port, "TEST_ZoneMissing")),
        NpcExtra::Weapon(port) => Some(TestLoadoutEntry::class(*port, ZONE_GUN)),
        NpcExtra::Pouch | NpcExtra::SecondWearer => None,
    }
}

/// Returns the Entities the extras name: the pouch and the weapon.
fn extra_entities() -> [TestEntity; 2] {
    [
        TestEntity::new(0x36, ZONE_POUCH, "entities/scitem/test_zone_pouch.xml")
            .attach_type("Char_Clothing_Backpack"),
        TestEntity::new(0x37, ZONE_GUN, "entities/scitem/test_zone_gun.xml")
            .attach_type("WeaponPersonal"),
    ]
}

/// Returns the head, the body and the spare body of the character.
fn character_entities(options: NpcZoneOptions, default_skin: &str) -> [TestEntity; 3] {
    let sex = if options.female { "Female" } else { "Male" };
    let head = TestEntity::new(
        0x31,
        "TEST_ZoneNpcHead",
        "entities/scitem/test_zone_head.xml",
    )
    .attach_type("Char_Head")
    .sub_type(options.head_sub_type.unwrap_or(sex));
    let body = TestEntity::new(
        0x32,
        "TEST_ZoneNpcBody",
        "entities/scitem/test_zone_body.xml",
    )
    .attach_type(options.body_type.unwrap_or("Char_Body"))
    .geometry(options.body_skin.unwrap_or(default_skin), None::<&str>);
    let body = if options.head_sub_type.is_some() {
        body.sub_type(sex)
    } else {
        body
    };
    let spare = TestEntity::new(
        0x34,
        "TEST_ZoneNpcSpareBody",
        "entities/scitem/test_zone_spare_body.xml",
    )
    .attach_type("Char_Body")
    .geometry(default_skin, None::<&str>);
    [head, body, spare]
}

fn npc_entries(
    bodies: [&[[f32; 2]]; 2],
    mut shirt: TestEntity,
    jacket: Option<TestEntity>,
    extras: &[NpcExtra],
    options: NpcZoneOptions,
) -> Vec<TestEntry> {
    let default_skin = if options.female {
        FEMALE_SKIN
    } else {
        MALE_SKIN
    };
    let characters = character_entities(options, default_skin);
    let mut worn = vec![shirt_entry(extras.contains(&NpcExtra::Pouch))];
    worn.extend(extras.iter().filter_map(extra_entry));
    if jacket.is_some() {
        worn.push(TestLoadoutEntry::class("Clothing_Torso_1", ZONE_JACKET));
    }
    if options.headless {
        shirt = shirt.geometry(format!("{MALE_ARMOUR}/m_suit.skin"), None::<&str>);
    } else {
        worn.insert(
            0,
            TestLoadoutEntry::class("Head_ItemPort", "TEST_ZoneNpcHead"),
        );
    }
    let mut ports = vec![
        TestLoadoutEntry::class("Body_ItemPort", "TEST_ZoneNpcBody")
            .loadout(TestLoadout::Manual(worn)),
    ];
    if options.second_body {
        ports.push(TestLoadoutEntry::class(
            "Spare_ItemPort",
            "TEST_ZoneNpcSpareBody",
        ));
    }
    if extras.contains(&NpcExtra::SecondWearer) {
        ports.push(
            TestLoadoutEntry::class("Second_ItemPort", "TEST_ZoneNpcSpareBody")
                .loadout(TestLoadout::Manual(vec![shirt_entry(false)])),
        );
    }
    let npc = TestEntity::new(0x33, NPC_ZONE_CLASS, "entities/scitem/test_zone_npc.xml")
        .loadout(TestLoadout::Manual(ports));
    let npc: Vec<TestEntity> = characters
        .into_iter()
        .chain([npc])
        .chain(extra_entities())
        .chain(jacket)
        .collect();
    let mut entries = zone_entries_with(bodies, &shirt, &npc);
    if let Some(skin) = options.body_skin {
        entries.extend(copies_of(&entries, default_skin, skin));
    }
    entries
}

fn copies_of(entries: &[TestEntry], from: &str, to: &str) -> Vec<TestEntry> {
    let stem = |path: &str| format!("Data\\{}", path.trim_end_matches(".skin")).replace('/', "\\");
    let (from, to) = (stem(from), stem(to));
    entries
        .iter()
        .filter_map(|entry| {
            let start = entry.path.get(..from.len())?;
            let rest = entry.path.get(from.len()..)?;
            start.eq_ignore_ascii_case(&from).then(|| {
                let mut copy = entry.clone();
                copy.path = format!("{to}{rest}");
                copy
            })
        })
        .collect()
}
