// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! What a real Archive shows about the colour of a stencil decal (ADR-0310): the values of
//! `StencilTintOverride`, and the channels of the stencil maps next to the second and third
//! stencil colour; runs with `--ignored` when `P4KX_DATA_P4K` names one.

use std::collections::BTreeMap;
use std::error::Error;
use std::io::Read;

use p4kx_extraction::model::{Material, MaterialLibrary};
use p4kx_extraction::{Archive, Entry, PathIndex, RgbaImage, Texture};
use p4kx_testkit::{real_archive_key, real_archive_path};
use rayon::prelude::*;

type TestResult = Result<(), Box<dyn Error>>;

const MAP_MIP_SIDE: u32 = 256;
const CHANNEL_LIVE: u8 = 32;
const OVERRIDE: &str = "StencilTintOverride";
const COLOUR: &str = "StencilDiffuseColor";
const SECOND_COLOUR: &str = "StencilDiffuseColor2";
const THIRD_COLOUR: &str = "StencilDiffuseColor3";

/// What a census counted: the keys of the materials it kept, and the libraries it could not read.
struct Census<K> {
    counts: BTreeMap<K, usize>,
    unreadable: usize,
}

/// Counts the keys `of` names for the materials of every material library of `archive`, read in
/// parallel.
fn census<K: Ord + Send>(
    archive: &Archive,
    of: impl Fn(&Material) -> Option<K> + Sync,
) -> Census<K> {
    let mtl: Vec<&Entry> = archive
        .entries()
        .iter()
        .filter(|entry| entry.path().to_ascii_lowercase().ends_with(".mtl"))
        .collect();
    let (counts, unreadable) = mtl
        .par_iter()
        .fold(
            || (BTreeMap::new(), 0_usize),
            |(mut counts, mut unreadable), entry| {
                let mut bytes = Vec::new();
                let read = archive
                    .open_entry(entry)
                    .ok()
                    .and_then(|mut reader| reader.read_to_end(&mut bytes).ok());
                match (read, MaterialLibrary::parse(entry.path(), &bytes)) {
                    (Some(_), Ok(library)) => {
                        for key in library.materials.iter().filter_map(&of) {
                            *counts.entry(key).or_default() += 1;
                        }
                    }
                    _ => unreadable += 1,
                }
                (counts, unreadable)
            },
        )
        .reduce(
            || (BTreeMap::new(), 0),
            |(mut all, unreadable), (part, part_unreadable)| {
                for (key, count) in part {
                    *all.entry(key).or_default() += count;
                }
                (all, unreadable + part_unreadable)
            },
        );
    Census { counts, unreadable }
}

/// Returns the entry path of the texture a material names as `file`.
fn entry_path(file: &str) -> String {
    let unified = file.replace('/', "\\");
    let stem = unified
        .rsplit_once('.')
        .map_or(unified.as_str(), |(stem, _)| stem);
    format!("Data\\{stem}.dds")
}

/// Returns which channels of `image` are live (above [`CHANNEL_LIVE`] somewhere), and whether the
/// green and blue channels ever differ from the red one by more than that.
fn live_channels(image: &RgbaImage) -> ([bool; 3], bool) {
    let mut live = [false; 3];
    let mut differs = false;
    let (pixels, _) = image.pixels().as_chunks::<4>();
    for [red, green, blue, _] in pixels {
        for (live, value) in live.iter_mut().zip([red, green, blue]) {
            *live |= *value > CHANNEL_LIVE;
        }
        differs |= green.abs_diff(*red) > CHANNEL_LIVE || blue.abs_diff(*red) > CHANNEL_LIVE;
    }
    (live, differs)
}

/// A stencil map next to the colours of the material that names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MapKind {
    /// The file is not in the Archive or does not decode.
    Missing,
    /// The file decodes.
    Read {
        live: [bool; 3],
        /// Green or blue differs from red: the map holds more than one mask.
        independent: bool,
        /// The material sets a second or third colour that is not its first.
        colours_apart: bool,
    },
}

/// Returns the kind of the stencil map of `material`, `None` when it has none.
fn map_kind(index: &PathIndex<'_>, material: &Material) -> Option<MapKind> {
    let file = &material.stencil_texture()?.file;
    let first = material.public_colour(COLOUR).unwrap_or(material.diffuse);
    let apart = |name| {
        material
            .public_colour(name)
            .is_some_and(|colour| colour.map(f32::to_bits) != first.map(f32::to_bits))
    };
    let image = Texture::from_index(index, &entry_path(file))
        .ok()
        .and_then(|texture| {
            let width = texture.header().width();
            let mip = (0..texture.header().mip_count())
                .find(|level| (width >> level) <= MAP_MIP_SIDE)
                .unwrap_or_else(|| texture.header().mip_count().saturating_sub(1));
            texture.decode_mip(mip).ok()
        });
    Some(image.map_or(MapKind::Missing, |image| {
        let (live, independent) = live_channels(&image);
        MapKind::Read {
            live,
            independent,
            colours_apart: apart(SECOND_COLOUR) || apart(THIRD_COLOUR),
        }
    }))
}

#[test]
#[expect(
    clippy::print_stdout,
    reason = "the census table is the measurement; the test prints it"
)]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_tint_override_of_a_stencil_decal_is_a_whole_number_that_is_not_only_0() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;

    let values = census(&archive, |material| {
        let (_, text) = material
            .public_params
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(OVERRIDE))?;
        material
            .is_stencil_decal()
            .then(|| text.trim().parse::<u32>().ok())
    });

    println!("{} libraries unreadable", values.unreadable);
    for (value, count) in &values.counts {
        println!("{count:>6}  {OVERRIDE}={value:?}");
    }
    assert!(
        values.counts.keys().all(Option::is_some),
        "every value is a non-negative integer"
    );
    for value in [0, 1] {
        assert!(values.counts.contains_key(&Some(value)), "{value}");
    }
    Ok(())
}

#[test]
#[expect(
    clippy::print_stdout,
    reason = "the census table is the measurement; the test prints it"
)]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_stencil_map_of_one_mask_can_have_second_and_third_colours_of_its_own() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let index = PathIndex::new(&archive);

    let kinds = census(&archive, |material| map_kind(&index, material));

    println!("{} libraries unreadable", kinds.unreadable);
    for (kind, count) in &kinds.counts {
        println!("{count:>6}  {kind:?}");
    }
    let has = |wanted: fn(&MapKind) -> bool| kinds.counts.keys().any(wanted);
    assert!(
        has(|kind| matches!(
            kind,
            MapKind::Read {
                live: [true, true, true],
                independent: false,
                colours_apart: true,
            }
        )),
        "a map of one mask in three channels, with colours of its own"
    );
    assert!(
        has(|kind| matches!(
            kind,
            MapKind::Read {
                independent: true,
                ..
            }
        )),
        "a map whose channels hold more than one mask"
    );
    Ok(())
}
