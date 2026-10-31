// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The logos of the RSI Ursa Medivac take the decal colours of a livery (ADR-0310): their
//! `StencilTintOverride` is 2, and every palette that states decal colours gives them to the decal.

use std::error::Error;

use p4kx_assembly::{
    ArchiveModels, Assembly, AssemblyOptions, ForgePalettes, LoadoutResolver, PaletteProvider,
};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_model::PaletteSlot;
use p4kx_testkit::{real_archive_key, real_archive_path};

type TestResult = Result<(), Box<dyn Error>>;

const URSA: &str = "RSI_Ursa_Medivac";
const TINT_PALETTE_TREE: &str = "TintPaletteTree";

#[test]
#[expect(
    clippy::print_stdout,
    reason = "the example livery is the measurement; the test prints it"
)]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn every_palette_with_decal_colours_paints_the_decals_of_the_ursa_medivac() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let models = ArchiveModels::new(&archive);
    let palettes = ForgePalettes::new(&forge);
    let mut resolver = LoadoutResolver::new(&forge);
    let record = resolver.entities().find(URSA)?;
    let tree = resolver.resolve(&record)?;
    let assembly =
        Assembly::build_with_palettes(&tree, &models, &palettes, &AssemblyOptions::default())?;
    let decals: Vec<_> = assembly
        .parts()
        .iter()
        .filter_map(|part| part.materials().library())
        .flat_map(|library| library.materials.iter())
        .filter(|material| material.palette == Some(PaletteSlot::Decal))
        .collect();
    assert!(
        !decals.is_empty(),
        "the Ursa Medivac has decals of override 2"
    );

    let (mut checked, mut example) = (0_usize, None);
    for record in forge.records().filter(|record| {
        record
            .type_name()
            .is_ok_and(|name| name == TINT_PALETTE_TREE)
    }) {
        let palette = palettes.by_guid(record.guid())?;
        let Some(decal) = palette.decal else {
            continue;
        };
        for material in &decals {
            let painted = palette
                .apply(material)
                .ok_or("the palette paints the decal")?;
            assert_eq!(painted.decal_colours, Some(decal), "{}", palette.name);
            assert_eq!(painted.diffuse, [1.0; 3], "{}", palette.name);
        }
        checked += 1;
        let bits = decal.map(|colour| colour.map(f32::to_bits));
        if example.is_none() && bits[0] != bits[1] && bits[1] != bits[2] && bits[0] != bits[2] {
            example = Some((palette.name.clone(), decal));
        }
    }

    println!(
        "{} decals, {checked} palettes; example: {example:?}",
        decals.len()
    );
    assert!(checked > 0, "palettes state decal colours");
    assert!(example.is_some(), "a livery sets three decal colours apart");
    Ok(())
}
