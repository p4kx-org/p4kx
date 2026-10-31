// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The body of an NPC Entity loses the zones its worn clothing covers, by the rule of the
//! character package.

use std::collections::BTreeSet;
use std::error::Error;

use p4kx_assembly::{Assembly, BodyVariant, IssueKind};
use p4kx_extraction::Archive;
use p4kx_testkit::{NPC_ZONE_CLASS, NpcZoneOptions, TestZoneChunk, npc_zone_entries};

use crate::assembly_support::{TestResult, assemble, fixture};

const BODY_PORT: &str = "Body_ItemPort";

fn places(variant: BodyVariant) -> Vec<[f32; 2]> {
    variant
        .zone_names()
        .iter()
        .map(|zone| {
            let x = if zone.starts_with("l_") {
                -0.2
            } else if zone.starts_with("r_") {
                0.2
            } else {
                0.0
            };
            [x, if zone.contains("foot") { 0.1 } else { 1.2 }]
        })
        .collect()
}

fn wear_on(
    bodies: &[Vec<[f32; 2]>; 2],
    chunks: &[TestZoneChunk],
    options: NpcZoneOptions,
) -> Result<Assembly, Box<dyn Error>> {
    let fixture = fixture(npc_zone_entries([&bodies[0], &bodies[1]], chunks, options))?;
    let archive = Archive::open(&fixture.path)?;
    assemble(&archive, NPC_ZONE_CLASS, 0)
}

fn wear(
    variant: BodyVariant,
    chunks: &[TestZoneChunk],
    headless: bool,
) -> Result<Assembly, Box<dyn Error>> {
    let options = NpcZoneOptions {
        female: variant == BodyVariant::Female,
        headless,
        ..NpcZoneOptions::default()
    };
    wear_on(&BodyVariant::ALL.map(places), chunks, options)
}

fn drawn_nodes(assembly: &Assembly) -> Result<BTreeSet<u16>, Box<dyn Error>> {
    let mesh = assembly
        .part_at(BODY_PORT)
        .and_then(|part| part.model())
        .and_then(|model| model.lods.first())
        .and_then(|lod| lod.meshes.first())
        .ok_or("no body")?;
    Ok(mesh.submeshes.iter().map(|part| part.node_id).collect())
}

fn node_of(variant: BodyVariant, zone: &str) -> Result<u16, Box<dyn Error>> {
    let at = variant
        .zone_names()
        .iter()
        .position(|name| *name == zone)
        .ok_or("no such zone")?;
    Ok(u16::try_from(at)?)
}

fn hidden_zones(assembly: &Assembly) -> Vec<Vec<String>> {
    assembly
        .issues()
        .iter()
        .filter_map(|issue| match issue.kind() {
            IssueKind::BodyZonesHidden(zones) => Some(zones.clone()),
            _ => None,
        })
        .collect()
}

fn has_zone_issue(assembly: &Assembly) -> bool {
    assembly.issues().iter().any(|issue| {
        matches!(
            issue.kind(),
            IssueKind::BodyZonesHidden(_) | IssueKind::BodyZonesUnknown(_)
        )
    })
}

#[test]
fn the_zones_a_worn_garment_covers_are_left_out_of_the_body() -> TestResult {
    for variant in BodyVariant::ALL {
        let chunks = [
            TestZoneChunk::new("torso01_zone", 1),
            TestZoneChunk::new("Torso03_Zone", 2),
        ];

        let assembly = wear(variant, &chunks, false)?;

        let drawn = drawn_nodes(&assembly)?;
        assert_eq!(drawn.len(), variant.zone_names().len() - 2);
        assert!(!drawn.contains(&node_of(variant, "torso01_zone")?));
        assert!(!drawn.contains(&node_of(variant, "torso03_zone")?));
        assert!(drawn.contains(&node_of(variant, "torso02_zone")?));
        assert_eq!(hidden_zones(&assembly), [["torso03_zone", "torso01_zone"]]);
    }
    Ok(())
}

#[test]
fn the_skin_of_a_generic_head_npc_is_cut_under_its_clothing() -> TestResult {
    for variant in BodyVariant::ALL {
        let options = NpcZoneOptions {
            female: variant == BodyVariant::Female,
            head_sub_type: Some("UNDEFINED"),
            ..NpcZoneOptions::default()
        };
        let chunks = [TestZoneChunk::new("torso01_zone", 1)];

        let assembly = wear_on(&BodyVariant::ALL.map(places), &chunks, options)?;

        let drawn = drawn_nodes(&assembly)?;
        assert_eq!(drawn.len(), variant.zone_names().len() - 1);
        assert!(!drawn.contains(&node_of(variant, "torso01_zone")?));
        assert_eq!(hidden_zones(&assembly), [["torso01_zone"]]);
    }
    Ok(())
}

#[test]
fn a_garment_that_covers_nothing_leaves_the_body_whole() -> TestResult {
    let chunks = [TestZoneChunk::new("l_arm04_zone", 1).showing(&[0])];

    let assembly = wear(BodyVariant::Male, &chunks, false)?;

    assert_eq!(
        drawn_nodes(&assembly)?.len(),
        BodyVariant::Male.zone_names().len()
    );
    assert!(!has_zone_issue(&assembly));
    Ok(())
}

#[test]
fn a_body_that_does_not_fit_the_zone_table_is_kept_whole_and_reported() -> TestResult {
    let chunks = [TestZoneChunk::new("torso01_zone", 1)];
    let few = vec![[0.0, 1.2]; 5];

    let assembly = wear_on(&[few.clone(), few], &chunks, NpcZoneOptions::default())?;

    assert_eq!(drawn_nodes(&assembly)?.len(), 5);
    assert_eq!(hidden_zones(&assembly), Vec::<Vec<String>>::new());
    let reasons: Vec<&str> = assembly
        .issues()
        .iter()
        .filter_map(|issue| match issue.kind() {
            IssueKind::BodyZonesUnknown(reason) => Some(reason.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(reasons.len(), 1);
    assert!(reasons[0].contains("5 zones"), "{reasons:?}");
    Ok(())
}

#[test]
fn a_body_whose_zones_lie_on_the_wrong_side_is_kept_whole_and_reported() -> TestResult {
    let chunks = [TestZoneChunk::new("torso01_zone", 1)];
    let mut male = places(BodyVariant::Male);
    let at = node_of(BodyVariant::Male, "l_arm01_zone")?;
    male[usize::from(at)][0] = 0.3;

    let assembly = wear_on(
        &[male, places(BodyVariant::Female)],
        &chunks,
        NpcZoneOptions::default(),
    )?;

    assert_eq!(
        drawn_nodes(&assembly)?.len(),
        BodyVariant::Male.zone_names().len()
    );
    assert!(assembly.issues().iter().any(|issue| matches!(
        issue.kind(),
        IssueKind::BodyZonesUnknown(reason) if reason.contains("l_arm01_zone")
    )));
    Ok(())
}

#[test]
fn a_tree_without_a_head_is_cut_by_the_body_its_body_entity_names() -> TestResult {
    let chunks = [TestZoneChunk::new("torso01_zone", 1)];

    for variant in BodyVariant::ALL {
        let assembly = wear(variant, &chunks, true)?;

        let drawn = drawn_nodes(&assembly)?;
        assert_eq!(drawn.len(), variant.zone_names().len() - 1);
        assert!(!drawn.contains(&node_of(variant, "torso01_zone")?));
        assert_eq!(hidden_zones(&assembly), [["torso01_zone"]]);
    }
    Ok(())
}

#[test]
fn a_tree_without_a_head_or_a_body_entity_keeps_the_body_whole_and_reports_no_zones() -> TestResult
{
    let chunks = [TestZoneChunk::new("torso01_zone", 1)];
    let options = NpcZoneOptions {
        headless: true,
        body_type: Some("Char_Clothing_Legs"),
        ..NpcZoneOptions::default()
    };

    let assembly = wear_on(&BodyVariant::ALL.map(places), &chunks, options)?;

    let shirt = assembly
        .part_at("Body_ItemPort/Clothing_Torso_0")
        .ok_or("no shirt")?;
    assert!(shirt.model().is_some(), "the garment must have a model");
    assert_eq!(
        drawn_nodes(&assembly)?.len(),
        BodyVariant::Male.zone_names().len()
    );
    assert!(!has_zone_issue(&assembly));
    Ok(())
}

#[test]
fn a_body_that_names_another_skin_file_is_cut_too() -> TestResult {
    let chunks = [TestZoneChunk::new("torso01_zone", 1)];
    let options = NpcZoneOptions {
        body_skin: Some("Objects/Characters/Human/male_v7/body/m_body_alt.skin"),
        ..NpcZoneOptions::default()
    };

    let assembly = wear_on(&BodyVariant::ALL.map(places), &chunks, options)?;

    assert_eq!(
        drawn_nodes(&assembly)?.len(),
        BodyVariant::Male.zone_names().len() - 1
    );
    assert_eq!(hidden_zones(&assembly), [["torso01_zone"]]);
    Ok(())
}

#[test]
fn a_cut_that_finds_no_body_part_is_reported() -> TestResult {
    let chunks = [TestZoneChunk::new("torso01_zone", 1)];
    let options = NpcZoneOptions {
        body_type: Some("Char_Clothing_Legs"),
        ..NpcZoneOptions::default()
    };

    let assembly = wear_on(&BodyVariant::ALL.map(places), &chunks, options)?;

    assert_eq!(
        drawn_nodes(&assembly)?.len(),
        BodyVariant::Male.zone_names().len()
    );
    assert_eq!(hidden_zones(&assembly), Vec::<Vec<String>>::new());
    assert!(assembly.issues().iter().any(|issue| matches!(
        issue.kind(),
        IssueKind::BodyZonesUnknown(reason) if reason.contains("Char_Body")
    )));
    Ok(())
}

fn unknown_reasons(assembly: &Assembly) -> Vec<&str> {
    assembly
        .issues()
        .iter()
        .filter_map(|issue| match issue.kind() {
            IssueKind::BodyZonesUnknown(reason) => Some(reason.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_body_skin_of_another_variant_is_kept_whole_and_reported() -> TestResult {
    let chunks = [TestZoneChunk::new("torso01_zone", 1)];
    let options = NpcZoneOptions {
        body_skin: Some("Objects/Characters/Human/other_v1/body/m_body.skin"),
        ..NpcZoneOptions::default()
    };

    let assembly = wear_on(&BodyVariant::ALL.map(places), &chunks, options)?;

    assert_eq!(
        drawn_nodes(&assembly)?.len(),
        BodyVariant::Male.zone_names().len()
    );
    assert_eq!(hidden_zones(&assembly), Vec::<Vec<String>>::new());
    let reasons = unknown_reasons(&assembly);
    assert_eq!(reasons.len(), 1);
    assert!(reasons[0].contains("variant"), "{reasons:?}");
    Ok(())
}

#[test]
fn several_body_parts_are_not_cut_and_reported() -> TestResult {
    let chunks = [TestZoneChunk::new("torso01_zone", 1)];
    let options = NpcZoneOptions {
        second_body: true,
        ..NpcZoneOptions::default()
    };

    let assembly = wear_on(&BodyVariant::ALL.map(places), &chunks, options)?;

    assert_eq!(
        drawn_nodes(&assembly)?.len(),
        BodyVariant::Male.zone_names().len()
    );
    assert_eq!(hidden_zones(&assembly), Vec::<Vec<String>>::new());
    let reasons = unknown_reasons(&assembly);
    assert_eq!(reasons.len(), 1);
    assert!(reasons[0].contains("several"), "{reasons:?}");
    Ok(())
}
