// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    ArchiveModels, Assembly, AssemblyOptions, ForgePalettes, LoadoutResolver, PaletteProvider,
    PaletteSelection,
};
use p4kx_extraction::{Archive, DataForgeEntry, SourceLocation};
use p4kx_testkit::livery_entries;

use crate::assembly_support::{TestResult, fixture};

use super::{assemble, assemble_ship};

#[test]
fn observed_palette_records_and_selection_survive_inheritance_and_default() -> TestResult {
    let fixture = fixture(livery_entries())?;
    let archive = Archive::open(&fixture.path)?;
    let assembly = assemble(&archive, true)?;
    let root = assembly
        .root()
        .and_then(p4kx_assembly::Part::palette_source)
        .ok_or("observed root palette")?;
    let child = assembly
        .part_at("hardpoint_default")
        .and_then(p4kx_assembly::Part::palette_source)
        .ok_or("inherited palette")?;
    assert_eq!(root.inherited_from(), None, "own authored palette");
    assert_eq!(
        child.inherited_from(),
        Some(0),
        "the original root part provided the inherited palette"
    );
    assert_eq!(
        root.records(),
        child.records(),
        "inherited from the same actual palette Record"
    );
    assert!(
        root.forge_origin()
            .is_some_and(|origin| origin.matches_archive(&archive)),
        "actual palette file belongs to the opened DataForge"
    );
    assert!(
        matches!(root.selection(), PaletteSelection::Named { location: Some(SourceLocation::DataForge { field, .. }) }
            if field.ends_with(".Geometry.Palette.RootRecord")),
        "use the geometry field that selected the GUID"
    );
    assert_eq!(
        root.named_origin(),
        child.named_origin(),
        "the inherited selector stays bound to the original geometry DataForge"
    );
    check_material_default(&archive)
}

fn check_material_default(archive: &Archive) -> TestResult {
    let bare = assemble_ship(archive, "TEST_Bare", true)?;
    let default = bare
        .part_at("hardpoint_default")
        .and_then(p4kx_assembly::Part::palette_source)
        .ok_or("material default palette")?;
    assert_eq!(
        default.inherited_from(),
        None,
        "library defaults do not become ancestor palettes"
    );
    assert!(
        matches!(default.selection(), PaletteSelection::MaterialDefault { library: Some(entry) }
            if archive.owns_origin(entry)),
        "the default is selected by a physical material library"
    );
    assert!(
        default.records().selector.is_some(),
        "path lookup Record is retained"
    );
    assert!(
        default.records().palette.is_some(),
        "the chosen palette Record is retained"
    );
    Ok(())
}

#[test]
fn palette_provider_does_not_bind_equivalent_dataforge_bytes_from_another_read() -> TestResult {
    let fixture = fixture(livery_entries())?;
    let archive = Archive::open(&fixture.path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let distinct = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    assert!(
        ForgePalettes::new(&forge)
            .with_entry(&distinct)
            .forge_origin()
            .is_none(),
        "equal decoded files do not turn an unobserved palette read into proved provenance"
    );
    assert!(
        ForgePalettes::new(&forge)
            .with_entry(&entry)
            .forge_origin()
            .is_some(),
        "exact opened DataForge input is accepted"
    );
    Ok(())
}

#[test]
fn a_palette_payload_can_come_from_a_different_dataforge_than_the_named_selector() -> TestResult {
    let entity_fixture = fixture(livery_entries())?;
    let palette_fixture = fixture(livery_entries())?;
    let entity_archive = Archive::open(&entity_fixture.path)?;
    let palette_archive = Archive::open(&palette_fixture.path)?;
    let entities = DataForgeEntry::read(&entity_archive)?;
    let palettes = DataForgeEntry::read(&palette_archive)?;
    let entity_forge = entities.open()?;
    let palette_forge = palettes.open()?;
    let mut resolver = LoadoutResolver::new(&entity_forge).with_forge_entry(&entities);
    let entity = resolver.entities().find("TEST_Livery")?;
    let selection = resolver.resolve(&entity)?;
    let assembly = Assembly::build_with_palettes(
        &selection,
        &ArchiveModels::new(&entity_archive),
        &ForgePalettes::new(&palette_forge).with_entry(&palettes),
        &AssemblyOptions::default(),
    )?;
    let observation = assembly
        .root()
        .and_then(p4kx_assembly::Part::palette_source)
        .ok_or("observed palette")?;
    assert!(
        observation
            .named_origin()
            .is_some_and(|origin| origin.matches_archive(&entity_archive)),
        "the authored model geometry supplies the palette selector"
    );
    assert!(
        observation
            .forge_origin()
            .is_some_and(|origin| origin.matches_archive(&palette_archive)),
        "the independently provided palette is bound to its own DataForge"
    );
    assert_ne!(
        observation.named_origin(),
        observation.forge_origin(),
        "equal DCB bytes and GUIDs preserve separate physical contributions"
    );
    Ok(())
}
