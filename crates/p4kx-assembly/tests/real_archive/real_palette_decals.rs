// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The decal colours of the tint palettes of a real Archive (ADR-0310): every `TintPaletteTree`
//! Record states `decalColorR`, `decalColorG` and `decalColorB`, which the masks of a stencil decal
//! with a tint override of 2 take.

use std::error::Error;

use p4kx_assembly::{ForgePalettes, PaletteProvider};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{real_archive_key, real_archive_path};

type TestResult = Result<(), Box<dyn Error>>;

const TINT_PALETTE_TREE: &str = "TintPaletteTree";
const WHITE_TOLERANCE: f32 = 1e-4;

fn is_white(colour: [f32; 3]) -> bool {
    colour
        .iter()
        .all(|channel| (channel - 1.0).abs() < WHITE_TOLERANCE)
}

#[test]
#[expect(
    clippy::print_stdout,
    reason = "the census is the measurement; the test prints it"
)]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn every_tint_palette_states_decal_colours_and_some_set_their_own() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let palettes = ForgePalettes::new(&forge);

    let mut count = 0_usize;
    let mut stated = 0_usize;
    let mut not_white = [0_usize; 3];
    let mut example = None;
    for record in forge.records().filter(|record| {
        record
            .type_name()
            .is_ok_and(|name| name == TINT_PALETTE_TREE)
    }) {
        count += 1;
        let palette = palettes.by_guid(record.guid())?;
        let Some(decal) = palette.decal else {
            continue;
        };
        stated += 1;
        for (found, colour) in not_white.iter_mut().zip(decal) {
            *found += usize::from(!is_white(colour));
        }
        if example.is_none() && decal.iter().any(|colour| !is_white(*colour)) {
            example = Some((palette.name.clone(), decal));
        }
    }

    println!("{count} palettes, {stated} state decal colours, not white R/G/B: {not_white:?}");
    println!("example: {example:?}");
    assert!(count > 0, "the Archive has tint palettes");
    assert_eq!(stated, count, "every palette states its decal colours");
    assert!(
        example.is_some(),
        "at least one livery sets a decal colour of its own"
    );
    Ok(())
}
