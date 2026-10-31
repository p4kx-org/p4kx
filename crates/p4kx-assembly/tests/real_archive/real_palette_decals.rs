// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The decal colours of the tint palettes of a real Archive (ADR-0310): every `TintPaletteTree`
//! Record states `decalColorR`, `decalColorG` and `decalColorB`, which the masks of a stencil decal
//! with a tint override of 2 take.

use std::error::Error;

use p4kx_dataforge::{Record, StructValue, Value};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{real_archive_key, real_archive_path};

type TestResult = Result<(), Box<dyn Error>>;

const TINT_PALETTE_TREE: &str = "TintPaletteTree";
const DECAL_COLOURS: [&str; 3] = ["decalColorR", "decalColorG", "decalColorB"];
const WHITE: [u8; 3] = [255; 3];

/// Returns the sRGB bytes of the colour field `name` of `root`.
fn colour(root: &StructValue<'_>, name: &str) -> Option<[u8; 3]> {
    let Some(Value::Struct(colour)) = root.field(name) else {
        return None;
    };
    let channel = |channel: &str| match colour.field(channel) {
        Some(Value::UInt8(byte)) => Some(*byte),
        _ => None,
    };
    Some([channel("r")?, channel("g")?, channel("b")?])
}

/// Returns the decal colours of `record`: red, green and blue.
fn decal_colours(record: &Record<'_>) -> Option<[[u8; 3]; 3]> {
    let tree = record.tree().ok()?;
    let Some(Value::Struct(root)) = tree.field("root") else {
        return None;
    };
    let [Some(red), Some(green), Some(blue)] = DECAL_COLOURS.map(|name| colour(root, name)) else {
        return None;
    };
    Some([red, green, blue])
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

    let mut palettes = 0_usize;
    let mut stated = 0_usize;
    let mut not_white = [0_usize; 3];
    let mut example = None;
    for record in forge.records().filter(|record| {
        record
            .type_name()
            .is_ok_and(|name| name == TINT_PALETTE_TREE)
    }) {
        palettes += 1;
        let Some(colours) = decal_colours(&record) else {
            continue;
        };
        stated += 1;
        for (count, decal) in not_white.iter_mut().zip(colours) {
            *count += usize::from(decal != WHITE);
        }
        if example.is_none() && colours.iter().any(|decal| *decal != WHITE) {
            example = Some((record.record_path().unwrap_or_default(), colours));
        }
    }

    println!("{palettes} palettes, {stated} state decal colours, not white R/G/B: {not_white:?}");
    println!("example: {example:?}");
    assert!(palettes > 0, "the Archive has tint palettes");
    assert_eq!(stated, palettes, "every palette states its decal colours");
    assert!(
        example.is_some(),
        "at least one livery sets a decal colour of its own"
    );
    Ok(())
}
