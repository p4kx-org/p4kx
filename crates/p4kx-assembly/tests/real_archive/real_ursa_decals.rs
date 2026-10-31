// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The logos of the RSI Ursa Medivac take the decal colours of a livery (ADR-0310): their
//! `StencilTintOverride` is 2, and a palette that states decal colours of its own gives them to the
//! decal.

use std::error::Error;

use p4kx_assembly::{
    ArchiveModels, Assembly, AssemblyOptions, ForgePalettes, LoadoutResolver, PaletteProvider,
};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_model::PaletteSlot;
use p4kx_testkit::{real_archive_key, real_archive_path};

type TestResult = Result<(), Box<dyn Error>>;

const URSA: &str = "RSI_Ursa_Medivac";
const LOGO: &str = "logo_decal";
const RESPITE: &str =
    "libs/foundry/records/tintpalettes/brand/rsi/medivac/rsi_ursa_medivac_black_white_grey_vip";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_logo_of_the_ursa_medivac_takes_the_decal_colours_of_a_livery() -> TestResult {
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
    let logo = assembly
        .parts()
        .iter()
        .filter_map(|part| part.materials().library())
        .flat_map(|library| library.materials.iter())
        .find(|material| material.name == LOGO)
        .ok_or("the logo decal")?;
    assert_eq!(logo.palette, Some(PaletteSlot::Decal));

    let livery = palettes.by_guid(palettes.guid_at(RESPITE)?)?;
    let decal = livery.decal.ok_or("the livery states decal colours")?;
    let bits = decal.map(|colour| colour.map(f32::to_bits));
    assert!(
        bits[0] != bits[1] && bits[1] != bits[2],
        "the livery sets decal colours of its own: {decal:?}"
    );
    let painted = livery.apply(logo).ok_or("the palette paints the logo")?;

    assert_eq!(painted.decal_colours, Some(decal));
    assert_eq!(painted.diffuse, [1.0; 3]);
    Ok(())
}
