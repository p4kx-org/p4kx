// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Entity Assembly leaves out a worn Entity whose port another worn Entity hides by its
//! `HiddenParts`, by the rules of the Armour (ADR-0279).

use std::collections::BTreeSet;
use std::error::Error;

use p4kx_assembly::{Assembly, BodyVariant, IssueKind};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    NPC_ZONE_CLASS, NpcExtra, TestZoneChunk, ZONE_JACKET, ZONE_SHIRT, npc_jacketed_entries,
};

use crate::assembly_support::{TestResult, assemble, fixture};
use crate::body_zones::places;

const SHIRT_PATH: &str = "Body_ItemPort/Clothing_Torso_0";
const JACKET_PATH: &str = "Body_ItemPort/Clothing_Torso_1";

type Chunks<'a> = &'a [TestZoneChunk];

fn wear(
    shirt: (Chunks<'_>, &[&str]),
    jacket: (Chunks<'_>, &[&str]),
) -> Result<Assembly, Box<dyn Error>> {
    wear_with(shirt, jacket, &[])
}

fn wear_with(
    shirt: (Chunks<'_>, &[&str]),
    jacket: (Chunks<'_>, &[&str]),
    extras: &[NpcExtra],
) -> Result<Assembly, Box<dyn Error>> {
    let [male, female] = BodyVariant::ALL.map(places);
    let entries = npc_jacketed_entries([&male, &female], shirt, jacket, extras);
    let fixture = fixture(entries)?;
    let archive = Archive::open(&fixture.path)?;
    assemble(&archive, NPC_ZONE_CLASS, 0)
}

fn is_unresolved(assembly: &Assembly) -> bool {
    assembly
        .issues()
        .iter()
        .any(|issue| matches!(issue.kind(), IssueKind::Unresolved(_)))
}

fn hidden_pieces(assembly: &Assembly) -> Vec<(&str, &str, &str)> {
    assembly
        .issues()
        .iter()
        .filter_map(|issue| match issue.kind() {
            IssueKind::PartHidden { port, by } => Some((issue.name(), port.as_str(), by.as_str())),
            _ => None,
        })
        .collect()
}

fn hidden_zones(assembly: &Assembly) -> Vec<BTreeSet<&str>> {
    assembly
        .issues()
        .iter()
        .filter_map(|issue| match issue.kind() {
            IssueKind::BodyZonesHidden(zones) => Some(zones.iter().map(String::as_str).collect()),
            _ => None,
        })
        .collect()
}

fn drawn_zones(assembly: &Assembly) -> Result<BTreeSet<&'static str>, Box<dyn Error>> {
    let mesh = assembly
        .part_at("Body_ItemPort")
        .and_then(|part| part.model())
        .and_then(|model| model.lods.first())
        .and_then(|lod| lod.meshes.first())
        .ok_or("no body")?;
    Ok(mesh
        .submeshes
        .iter()
        .filter_map(|part| {
            BodyVariant::Male
                .zone_names()
                .get(usize::from(part.node_id))
        })
        .copied()
        .collect())
}

fn is_drawn(assembly: &Assembly, path: &str) -> bool {
    assembly
        .part_at(path)
        .is_some_and(|part| part.model().is_some() && !part.is_hidden())
}

#[test]
fn a_garment_at_a_port_another_garment_hides_is_left_out_and_reported() -> TestResult {
    let shirt = [TestZoneChunk::new("torso01_zone", 1)];
    let jacket = [TestZoneChunk::new("l_arm01_zone", 2)];

    let assembly = wear((&shirt, &[]), (&jacket, &["clothing_torso_0"]))?;

    let hidden = assembly.part_at(SHIRT_PATH).ok_or("no shirt")?;
    assert!(hidden.is_hidden() && hidden.model().is_none());
    assert!(is_drawn(&assembly, JACKET_PATH));
    assert_eq!(
        hidden_pieces(&assembly),
        [(
            ZONE_SHIRT,
            "Clothing_Torso_0",
            format!("Clothing_Torso_1/{ZONE_JACKET}").as_str()
        )]
    );
    Ok(())
}

#[test]
fn a_hidden_garment_covers_no_zone_of_the_body() -> TestResult {
    let shirt = [TestZoneChunk::new("torso01_zone", 1)];
    let jacket = [TestZoneChunk::new("l_arm01_zone", 2)];

    let assembly = wear((&shirt, &[]), (&jacket, &["Clothing_Torso_0"]))?;

    assert_eq!(hidden_zones(&assembly), [BTreeSet::from(["l_arm01_zone"])]);
    let drawn = drawn_zones(&assembly)?;
    assert!(drawn.contains("torso01_zone"));
    assert!(!drawn.contains("l_arm01_zone"));
    Ok(())
}

#[test]
fn garments_that_hide_nothing_are_all_worn() -> TestResult {
    let shirt = [TestZoneChunk::new("torso01_zone", 1)];
    let jacket = [TestZoneChunk::new("l_arm01_zone", 2)];

    let assembly = wear((&shirt, &[]), (&jacket, &["Hair_ItemPort"]))?;

    assert!(is_drawn(&assembly, SHIRT_PATH) && is_drawn(&assembly, JACKET_PATH));
    assert_eq!(hidden_pieces(&assembly), []);
    assert_eq!(
        hidden_zones(&assembly),
        [BTreeSet::from(["l_arm01_zone", "torso01_zone"])]
    );
    Ok(())
}

#[test]
fn a_garment_never_hides_itself() -> TestResult {
    let shirt = [TestZoneChunk::new("torso01_zone", 1)];

    let assembly = wear((&shirt, &["clothing_torso_0"]), (&[], &[]))?;

    assert!(is_drawn(&assembly, SHIRT_PATH));
    assert_eq!(hidden_pieces(&assembly), []);
    Ok(())
}

const JACKET_PART: &str = "Clothing_Torso_1/TEST_ZoneJacket";

#[test]
fn a_weapon_at_a_port_named_like_a_hidden_clothing_port_stays_drawn() -> TestResult {
    let jacket = [TestZoneChunk::new("l_arm01_zone", 2)];

    let assembly = wear_with(
        (&[], &[]),
        (&jacket, &["Clothing_Legs"]),
        &[NpcExtra::Weapon("Clothing_Legs")],
    )?;

    let gun = assembly
        .part_at("Body_ItemPort/Clothing_Legs")
        .ok_or("no weapon")?;
    assert!(!gun.is_hidden());
    assert_eq!(hidden_pieces(&assembly), []);
    Ok(())
}

#[test]
fn the_clothing_of_one_wearer_does_not_hide_the_items_of_another() -> TestResult {
    let jacket = [TestZoneChunk::new("l_arm01_zone", 2)];

    let assembly = wear_with(
        (&[], &[]),
        (&jacket, &["Clothing_Torso_0"]),
        &[NpcExtra::SecondWearer],
    )?;

    let own = assembly.part_at(SHIRT_PATH).ok_or("no shirt")?;
    let other = assembly
        .part_at("Second_ItemPort/Clothing_Torso_0")
        .ok_or("no shirt of the second wearer")?;
    assert!(own.is_hidden() && !other.is_hidden());
    assert_eq!(
        hidden_pieces(&assembly),
        [(ZONE_SHIRT, "Clothing_Torso_0", JACKET_PART)]
    );
    Ok(())
}

#[test]
fn a_part_below_a_hidden_garment_is_hidden_with_it() -> TestResult {
    let jacket = [TestZoneChunk::new("l_arm01_zone", 2)];

    let assembly = wear_with(
        (&[], &[]),
        (&jacket, &["Clothing_Torso_0"]),
        &[NpcExtra::Pouch],
    )?;

    let pouch = assembly
        .part_at("Body_ItemPort/Clothing_Torso_0/Pouch_ItemPort")
        .ok_or("no pouch")?;
    assert!(pouch.is_hidden() && pouch.model().is_none());
    assert_eq!(
        hidden_pieces(&assembly),
        [
            (ZONE_SHIRT, "Clothing_Torso_0", JACKET_PART),
            ("TEST_ZonePouch", "Pouch_ItemPort", JACKET_PART),
        ]
    );
    Ok(())
}

#[test]
fn an_unresolved_attachment_at_a_hidden_port_is_reported_as_hidden() -> TestResult {
    let jacket = [TestZoneChunk::new("l_arm01_zone", 2)];
    let missing = [NpcExtra::Missing("Hair_ItemPort")];

    let hidden = wear_with((&[], &[]), (&jacket, &["hair_itemport"]), &missing)?;
    let kept = wear_with((&[], &[]), (&jacket, &[]), &missing)?;

    assert!(!is_unresolved(&hidden));
    assert_eq!(
        hidden_pieces(&hidden),
        [("TEST_ZoneMissing", "Hair_ItemPort", JACKET_PART)]
    );
    assert!(is_unresolved(&kept));
    assert_eq!(hidden_pieces(&kept), []);
    Ok(())
}

#[test]
fn two_garments_that_hide_each_other_are_both_left_out() -> TestResult {
    let shirt = [TestZoneChunk::new("torso01_zone", 1)];
    let jacket = [TestZoneChunk::new("l_arm01_zone", 2)];

    let assembly = wear(
        (&shirt, &["Clothing_Torso_1"]),
        (&jacket, &["Clothing_Torso_0"]),
    )?;

    assert!(!is_drawn(&assembly, SHIRT_PATH) && !is_drawn(&assembly, JACKET_PATH));
    assert_eq!(hidden_pieces(&assembly).len(), 2);
    assert_eq!(hidden_zones(&assembly), Vec::<BTreeSet<&str>>::new());
    Ok(())
}
