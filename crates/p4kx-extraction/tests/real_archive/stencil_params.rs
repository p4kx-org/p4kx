// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! What a real Archive leaves open about the colour of a stencil decal (ADR-0310): the values of
//! `StencilTintOverride`, and whether the green and blue channels of a stencil map select
//! `StencilDiffuseColor2` and `StencilDiffuseColor3`; runs with `--ignored` when `P4KX_DATA_P4K`
//! names one.

use std::collections::{BTreeMap, BTreeSet};
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

/// Counts the keys `of` names for the materials of every material library of `archive`, read in
/// parallel.
fn census<K: Ord + Send>(
    archive: &Archive,
    of: impl Fn(&Material) -> Option<K> + Sync,
) -> BTreeMap<K, usize> {
    let mtl: Vec<&Entry> = archive
        .entries()
        .iter()
        .filter(|entry| entry.path().to_ascii_lowercase().ends_with(".mtl"))
        .collect();
    mtl.par_iter()
        .fold(BTreeMap::new, |mut counts, entry| {
            let mut bytes = Vec::new();
            let read = archive
                .open_entry(entry)
                .ok()
                .and_then(|mut reader| reader.read_to_end(&mut bytes).ok());
            if let (Some(_), Ok(library)) = (read, MaterialLibrary::parse(entry.path(), &bytes)) {
                for key in library.materials.iter().filter_map(&of) {
                    *counts.entry(key).or_default() += 1;
                }
            }
            counts
        })
        .reduce(BTreeMap::new, |mut all, part| {
            for (key, count) in part {
                *all.entry(key).or_default() += count;
            }
            all
        })
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

#[test]
#[expect(
    clippy::print_stdout,
    reason = "the census table is the measurement; the test prints it"
)]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_tint_override_of_a_stencil_decal_takes_the_values_0_1_and_2() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;

    let values = census(&archive, |material| {
        let value = material
            .public_params
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(OVERRIDE))?
            .1
            .clone();
        material.is_stencil_decal().then_some(value)
    });

    for (value, count) in &values {
        println!("{count:>6}  {OVERRIDE}={value}");
    }
    let found: BTreeSet<&str> = values.keys().map(String::as_str).collect();
    assert_eq!(found, BTreeSet::from(["0", "1", "2"]));
    Ok(())
}

#[test]
#[expect(
    clippy::print_stdout,
    reason = "the census table is the measurement; the test prints it"
)]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_grey_stencil_map_has_second_and_third_colours_of_its_own() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let index = PathIndex::new(&archive);

    let materials = census(&archive, |material| {
        let file = material.stencil_texture()?.file.clone();
        let first = material.public_colour(COLOUR);
        let apart = |name| {
            material
                .public_colour(name)
                .is_some_and(|c| Some(c) != first)
        };
        Some((file, apart(SECOND_COLOUR) || apart(THIRD_COLOUR)))
    });

    let mut kinds = BTreeMap::<String, usize>::new();
    for ((file, colours_apart), count) in &materials {
        let kind = Texture::from_index(&index, &entry_path(file))
            .ok()
            .and_then(|texture| {
                let width = texture.header().width();
                let mip = (0..texture.header().mip_count())
                    .find(|level| (width >> level) <= MAP_MIP_SIDE)
                    .unwrap_or_else(|| texture.header().mip_count().saturating_sub(1));
                texture.decode_mip(mip).ok()
            })
            .map_or_else(
                || "no texture".to_owned(),
                |image| {
                    let (live, differs) = live_channels(&image);
                    format!(
                        "live={live:?} green_blue_differ={differs} colours_apart={colours_apart}"
                    )
                },
            );
        *kinds.entry(kind).or_default() += count;
    }

    for (kind, count) in &kinds {
        println!("{count:>6}  {kind}");
    }
    assert!(
        kinds.contains_key("live=[true, true, true] green_blue_differ=false colours_apart=true"),
        "a map whose three channels hold one mask, with three colours"
    );
    Ok(())
}
