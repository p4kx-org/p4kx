// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Tint palettes and textures of the synthetic ship `TEST_Livery`: which palette each part takes,
//! what could not be resolved and how the GLB draws the materials.

use crate::assembly_support::{TestResult, fixture};
use p4kx_assembly::{
    ArchiveModels, Assembly, AssemblyOptions, ForgePalettes, IssueKind, LoadoutResolver,
};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_gltf::{GlbOptions, GlbSummary, MaterialMode};
use p4kx_model::srgb8_to_linear;
use p4kx_testkit::{
    CheckedGlb, LIVERY_DEFAULT_TINTS, LIVERY_GUN_TINTS, LIVERY_SHIP_DECALS, LIVERY_SHIP_TINTS,
    check_glb, livery_entries,
};

fn linear(bytes: [u8; 3]) -> [f32; 3] {
    bytes.map(srgb8_to_linear)
}

fn assemble(archive: &Archive, palettes: bool) -> Result<Assembly, Box<dyn std::error::Error>> {
    assemble_ship(archive, "TEST_Livery", palettes)
}

fn assemble_ship(
    archive: &Archive,
    class: &str,
    palettes: bool,
) -> Result<Assembly, Box<dyn std::error::Error>> {
    let entry = DataForgeEntry::read(archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge);
    let record = resolver.entities().find(class)?;
    let tree = resolver.resolve(&record)?;
    let models = ArchiveModels::new(archive);
    let options = AssemblyOptions::default();
    Ok(if palettes {
        Assembly::build_with_palettes(&tree, &models, &ForgePalettes::new(&forge), &options)?
    } else {
        Assembly::build(&tree, &models, &options)?
    })
}

fn palette_name(assembly: &Assembly, path: &str) -> Option<String> {
    assembly
        .part_at(path)?
        .palette()
        .map(|palette| palette.name.clone())
}

fn colours(glb: &CheckedGlb) -> Vec<[f32; 3]> {
    glb.document
        .materials()
        .map(|material| {
            let [r, g, b, _] = material.pbr_metallic_roughness().base_color_factor();
            [r, g, b]
        })
        .collect()
}

fn has_colour(colours: &[[f32; 3]], wanted: [f32; 3]) -> bool {
    colours
        .iter()
        .any(|colour| colour.iter().zip(wanted).all(|(a, w)| (a - w).abs() < 1e-4))
}

#[test]
fn each_part_takes_the_palette_its_entity_names_else_the_one_of_its_ancestor() -> TestResult {
    let fixture = fixture(livery_entries())?;
    let archive = Archive::open(&fixture.path)?;

    let assembly = assemble(&archive, true)?;

    let names = |path: &str| palette_name(&assembly, path);
    assert_eq!(
        assembly
            .root()
            .and_then(|r| r.palette())
            .map(|p| p.name.as_str()),
        Some("p_ship")
    );
    assert_eq!(names("hardpoint_painted").as_deref(), Some("p_gun"));
    assert_eq!(names("hardpoint_twin").as_deref(), Some("p_gun"));
    assert_eq!(
        names("hardpoint_default").as_deref(),
        Some("p_ship"),
        "the ship's palette beats the library's default"
    );
    assert_eq!(names("hardpoint_plain").as_deref(), Some("p_ship"));
    let primary = assembly
        .root()
        .and_then(|root| root.palette())
        .map(|palette| palette.primary.tint);
    assert_eq!(primary, Some(linear(LIVERY_SHIP_TINTS[0])));
    Ok(())
}

#[test]
fn a_palette_states_the_decal_colours_of_its_record() -> TestResult {
    let fixture = fixture(livery_entries())?;
    let archive = Archive::open(&fixture.path)?;

    let assembly = assemble(&archive, true)?;

    let decal = assembly
        .root()
        .and_then(|root| root.palette())
        .and_then(|palette| palette.decal);

    assert_eq!(decal, Some(LIVERY_SHIP_DECALS.map(linear)));
    Ok(())
}

#[test]
fn without_a_palette_of_the_ancestors_a_part_keeps_the_default_of_its_library() -> TestResult {
    let fixture = fixture(livery_entries())?;
    let archive = Archive::open(&fixture.path)?;

    let assembly = assemble_ship(&archive, "TEST_Bare", true)?;

    let names = |path: &str| palette_name(&assembly, path);
    assert_eq!(names(""), None);
    assert_eq!(names("hardpoint_painted").as_deref(), Some("p_gun"));
    assert_eq!(names("hardpoint_default").as_deref(), Some("p_default"));
    let default = assembly
        .part_at("hardpoint_default")
        .and_then(|part| part.palette())
        .map(|palette| palette.primary.tint);
    assert_eq!(default, Some(linear(LIVERY_DEFAULT_TINTS[0])));
    assert_eq!(names("hardpoint_plain"), None, "no palette anywhere");
    Ok(())
}

#[test]
fn a_palette_is_inherited_through_parts_that_have_none_and_a_part_of_its_own_keeps_it() -> TestResult
{
    let fixture = fixture(livery_entries())?;
    let archive = Archive::open(&fixture.path)?;

    let assembly = assemble_ship(&archive, "TEST_Carrier", true)?;

    let names = |path: &str| palette_name(&assembly, path);
    assert_eq!(
        names("hardpoint_mount").as_deref(),
        Some("p_ship"),
        "inherited over the library default"
    );
    assert_eq!(
        names("hardpoint_mount/hardpoint_inner").as_deref(),
        Some("p_ship"),
        "inherited through the mount"
    );
    assert_eq!(
        names("hardpoint_mount/hardpoint_gun").as_deref(),
        Some("p_gun"),
        "its own palette is kept"
    );
    Ok(())
}

#[test]
fn the_parts_of_one_palette_share_it() -> TestResult {
    let fixture = fixture(livery_entries())?;
    let archive = Archive::open(&fixture.path)?;

    let assembly = assemble(&archive, true)?;

    let of = |path: &str| assembly.part_at(path).and_then(|part| part.palette());
    let (first, second) = (of("hardpoint_painted"), of("hardpoint_twin"));
    assert!(first.is_some());
    assert!(
        first.zip(second).is_some_and(|(a, b)| std::ptr::eq(a, b)),
        "one palette, read once"
    );
    Ok(())
}

#[test]
fn a_palette_that_cannot_be_resolved_is_listed_and_the_part_takes_the_one_of_its_ancestor()
-> TestResult {
    let fixture = fixture(livery_entries())?;
    let archive = Archive::open(&fixture.path)?;

    let assembly = assemble(&archive, true)?;

    let palette_issues: Vec<(&str, String)> = assembly
        .issues()
        .iter()
        .filter(|issue| matches!(issue.kind(), IssueKind::Palette { .. }))
        .map(|issue| (issue.path(), issue.kind().to_string()))
        .collect();
    assert_eq!(palette_issues.len(), 2, "{palette_issues:?}");
    let lost = palette_issues
        .iter()
        .find(|(path, _)| *path == "hardpoint_lost")
        .ok_or("the lost palette")?;
    assert!(lost.1.contains("does not exist"), "{lost:?}");
    let child = palette_issues
        .iter()
        .find(|(path, _)| *path == "hardpoint_child")
        .ok_or("the child palette")?;
    assert!(child.1.contains("left"), "{child:?}");
    for path in ["hardpoint_lost", "hardpoint_child"] {
        assert_eq!(
            palette_name(&assembly, path).as_deref(),
            Some("p_ship"),
            "the part falls back to the palette of the ship"
        );
    }
    assert!(
        assembly
            .issues()
            .iter()
            .filter(|issue| matches!(issue.kind(), IssueKind::Palette { .. }))
            .all(|issue| !issue.kind().skips()),
        "a palette never drops a part"
    );
    Ok(())
}

#[test]
fn without_palettes_no_part_has_one_and_nothing_is_reported() -> TestResult {
    let fixture = fixture(livery_entries())?;
    let archive = Archive::open(&fixture.path)?;

    let assembly = assemble(&archive, false)?;

    assert!(assembly.parts().iter().all(|part| part.palette().is_none()));
    assert!(
        !assembly
            .issues()
            .iter()
            .any(|issue| matches!(issue.kind(), IssueKind::Palette { .. }))
    );
    Ok(())
}

fn export(
    palettes: bool,
    options: &GlbOptions,
) -> Result<(CheckedGlb, GlbSummary), Box<dyn std::error::Error>> {
    let fixture = fixture(livery_entries())?;
    let archive = Archive::open(&fixture.path)?;
    let assembly = assemble(&archive, palettes)?;
    let models = ArchiveModels::new(&archive);
    let mut bytes = Vec::new();
    let summary = assembly.write_glb_with(options, &models.textures(), &mut bytes)?;
    Ok((check_glb(&bytes)?, summary))
}

#[test]
fn the_materials_are_drawn_with_the_colours_of_the_palette_of_their_part() -> TestResult {
    let (glb, summary) = export(true, &GlbOptions::default())?;

    let colours = colours(&glb);
    for tints in [LIVERY_SHIP_TINTS, LIVERY_GUN_TINTS] {
        for tint in &tints[..2] {
            assert!(
                has_colour(&colours, linear(*tint)),
                "{tint:?} in {colours:?}"
            );
        }
    }
    assert_eq!(
        summary.shared_meshes, 3,
        "the second painted gun and the parts that inherited the ship's palette, but not \
         the guns of other palettes: {summary:?}"
    );
    Ok(())
}

#[test]
fn without_palettes_every_part_has_the_colours_of_its_library() -> TestResult {
    let (glb, _) = export(false, &GlbOptions::default())?;

    assert!(
        colours(&glb)
            .iter()
            .all(|colour| colour.iter().all(|channel| (channel - 0.5).abs() < 1e-5))
    );
    Ok(())
}

#[test]
fn textures_are_embedded_once_for_all_parts_and_palettes() -> TestResult {
    let options = GlbOptions::default().with_materials(MaterialMode::Textures);

    let (glb, summary) = export(true, &options)?;

    assert_eq!(
        summary.textures, 1,
        "one diffuse image for every part: {summary:?}"
    );
    assert_eq!(glb.document.images().len(), 1);
    assert!(summary.textured_materials >= 4, "{summary:?}");
    assert!(
        summary.texture_fallbacks.is_empty(),
        "{:?}",
        summary.texture_fallbacks
    );
    assert!(summary.shared_textures > 0);
    Ok(())
}

#[test]
fn the_texture_mip_picks_the_size_of_the_embedded_image() -> TestResult {
    let largest = GlbOptions::default().with_materials(MaterialMode::Textures);

    let (_, big) = export(true, &largest)?;
    let (_, small) = export(true, &largest.with_texture_mip(2))?;
    let (_, past_the_end) = export(true, &largest.with_texture_mip(9))?;

    assert_eq!(big.texture_pixels, 64, "8 by 8");
    assert_eq!(small.texture_pixels, 4, "2 by 2");
    assert_eq!(past_the_end.texture_pixels, 1, "the smallest mip");
    Ok(())
}
