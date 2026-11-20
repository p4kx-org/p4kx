// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A worn item without bone weights hangs on the bone of the port it is attached to.

use std::error::Error;

use p4kx_assembly::{ArchiveModels, Armour, AssemblyOptions, BodyVariant, LoadoutResolver, Piece};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_gltf::GlbOptions;
use p4kx_testkit::{
    CheckedGlb, MOUNT_BONE, MOUNT_COURIER, MOUNT_DRIFTER, SATCHEL_HELPER_Z, check_glb,
    mounted_item_entries, mounted_overlay_entries,
};

use crate::assembly_support::{TestResult, fixture};

fn wear(character: &str) -> Result<Armour, Box<dyn Error>> {
    let fixture = fixture(mounted_item_entries())?;
    let archive = Archive::open(&fixture.path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge);
    let record = resolver.entities().find(character)?;
    let tree = resolver.resolve(&record)?;
    let pieces = resolver.worn(&tree);
    Ok(Armour::build(
        BodyVariant::Male,
        &pieces,
        &ArchiveModels::new(&archive),
        &AssemblyOptions::default(),
    )?)
}

fn glb_of(armour: &Armour) -> Result<CheckedGlb, Box<dyn Error>> {
    let mut bytes = Vec::new();
    armour.write_glb(&GlbOptions::default(), &mut bytes)?;
    Ok(check_glb(&bytes)?)
}

fn placement(glb: &CheckedGlb, name: &str) -> Result<([f32; 3], [f32; 4]), Box<dyn Error>> {
    let found = glb
        .document
        .nodes()
        .find(|candidate| candidate.name() == Some(name))
        .ok_or_else(|| format!("no node {name}"))?;
    let (translation, rotation, _) = found.transform().decomposed();
    Ok((translation, rotation))
}

fn parent_name<'a>(glb: &'a CheckedGlb, name: &str) -> Option<&'a str> {
    glb.document
        .nodes()
        .find(|parent| parent.children().any(|child| child.name() == Some(name)))
        .and_then(|parent| parent.name())
}

fn close(actual: &[f32], expected: &[f32]) -> bool {
    actual
        .iter()
        .zip(expected)
        .all(|(a, e)| (a - e).abs() < 1e-5)
}

#[test]
fn a_backpack_hangs_on_the_bone_its_port_names_and_follows_it() -> TestResult {
    let armour = wear(MOUNT_COURIER)?;
    let glb = glb_of(&armour)?;

    assert_eq!(
        parent_name(&glb, "satchel"),
        Some(MOUNT_BONE),
        "the nearest bone of the body above the bone of the armour"
    );
    let (translation, rotation) = placement(&glb, "satchel")?;
    assert!(
        close(&translation, &[0.0, 0.6, -SATCHEL_HELPER_Z + 0.2]),
        "the bone, the port's offset and the satchel's own helper: {translation:?}"
    );
    assert!(
        close(
            &rotation,
            &[
                std::f32::consts::FRAC_1_SQRT_2,
                0.0,
                0.0,
                std::f32::consts::FRAC_1_SQRT_2
            ]
        ),
        "the helper's axes are turned onto the bone's: {rotation:?}"
    );
    assert_eq!(
        parent_name(&glb, "satchel_loc"),
        Some("satchel"),
        "its helper stays below it"
    );
    Ok(())
}

#[test]
fn a_placed_backpack_is_not_listed_as_unskinned() -> TestResult {
    let armour = wear(MOUNT_COURIER)?;

    let codes: Vec<&str> = armour
        .issues()
        .iter()
        .map(|issue| issue.kind().code())
        .collect();
    assert!(!codes.contains(&"not_skinned"), "{codes:?}");
    assert!(!codes.contains(&"helper_not_found"), "{codes:?}");
    Ok(())
}

#[test]
fn a_port_whose_bone_the_armour_lacks_is_listed_and_the_backpack_stays_at_the_origin() -> TestResult
{
    let armour = wear(MOUNT_DRIFTER)?;
    let glb = glb_of(&armour)?;

    let issue = armour
        .issues()
        .iter()
        .find(|issue| issue.kind().code() == "helper_not_found")
        .ok_or("the missing helper is listed")?;
    assert_eq!(issue.name(), "pack/Mount_Satchel");
    assert!(
        issue.kind().to_string().contains("nowhere"),
        "{}",
        issue.kind()
    );
    assert_eq!(parent_name(&glb, "satchel"), None, "a root of the scene");
    Ok(())
}

fn wear_overlay(holder: &str, replace_item: bool) -> Result<Armour, Box<dyn Error>> {
    let fixture = fixture(mounted_overlay_entries())?;
    let archive = Archive::open(&fixture.path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge);
    let character = resolver.entities().find(MOUNT_COURIER)?;
    let tree = resolver.resolve(&character)?;
    let holder = resolver.entities().find(holder)?;
    let mut pieces = vec![Piece::wearable(resolver.wearable(&holder)?)];
    if replace_item {
        let item = resolver.entities().find("Mount_Explicit_Satchel")?;
        pieces.push(Piece::wearable(resolver.wearable(&item)?));
    }
    let (pieces, replaced) = resolver.worn_layered(&tree, pieces);
    assert_eq!(
        replaced.len(),
        if replace_item { 2 } else { 1 },
        "replacing the torso retains the normal piece-replacement report"
    );
    assert_eq!(
        replaced.first().map(p4kx_assembly::Replaced::name),
        Some("torso/Mount_Carrier"),
        "the report retains the old torso occurrence"
    );
    Ok(Armour::build(
        BodyVariant::Male,
        &pieces,
        &ArchiveModels::new(&archive),
        &AssemblyOptions::default(),
    )?)
}

#[test]
fn a_replaced_holder_uses_its_current_port_for_a_retained_or_explicit_piece() -> TestResult {
    for replace_item in [false, true] {
        let armour = wear_overlay("Mount_New_Carrier", replace_item)?;
        let glb = glb_of(&armour)?;
        assert_eq!(parent_name(&glb, "satchel"), Some(MOUNT_BONE));
        let (translation, _) = placement(&glb, "satchel")?;
        assert!(
            close(&translation, &[0.0, 0.6, -SATCHEL_HELPER_Z + 0.8]),
            "the new torso's port contributes the replacement offset: {translation:?}"
        );
        assert!(
            armour
                .issues()
                .iter()
                .all(|issue| issue.kind().code() != "worn_mount_unresolved"),
            "a unique compatible slot was bound"
        );
    }
    Ok(())
}

#[test]
fn a_conflicting_or_incompatible_replacement_never_writes_the_rigid_piece_at_the_origin()
-> TestResult {
    for (holder, reason) in [
        ("Mount_Duplicate_Carrier", "competing"),
        ("Mount_Incompatible_Carrier", "item type, subtype or size"),
    ] {
        let armour = wear_overlay(holder, false)?;
        assert!(
            placement(&glb_of(&armour)?, "satchel").is_err(),
            "the rigid geometry is omitted"
        );
        let issue = armour
            .issues()
            .iter()
            .find(|issue| issue.kind().code() == "worn_mount_unresolved")
            .ok_or("a scoped mount failure")?;
        assert!(issue.kind().skips());
        assert_eq!(issue.name(), "pack/Mount_Satchel");
        assert!(issue.kind().to_string().contains(reason), "{issue:?}");
        assert!(
            armour.parts().iter().any(|part| part.name() == holder),
            "the skinned holder remains"
        );
    }
    Ok(())
}
