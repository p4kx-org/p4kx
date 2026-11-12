// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Textured materials of a real Archive; runs with `--ignored` when `P4KX_DATA_P4K` names one.
//! The models, material libraries and textures read here are not encrypted, so the tests need no
//! Archive key.

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::io::Read;
use std::path::Path;

use p4kx_extraction::model::MaterialLibrary;
use p4kx_extraction::{
    Archive, Entry, GlbOptions, MaterialMode, ModelSource, PathIndex, RgbaImage, Texture,
};
use p4kx_testkit::{check_glb, real_archive_path};
use rayon::prelude::*;

type TestResult = Result<(), Box<dyn Error>>;

const SAMPLE_PER_EXTENSION: usize = 120;
const TEXTURE_MIP: u32 = 3;
const NORMAL_MAP_SAMPLE_STRIDE: usize = 7;
const NORMAL_MAP_SIDE: u32 = 256;
const MIN_INTEGRABLE: f64 = 0.2;
const MIN_DIRECTX_SHARE: f64 = 0.8;
const MIN_NORMAL_MAPS: usize = 200;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_sample_of_models_exports_valid_textured_glb() -> TestResult {
    let archive = Archive::open_with_key(
        real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let index = PathIndex::new(&archive);
    let out = tempfile::tempdir()?;
    let mut sample: Vec<&Entry> = Vec::new();
    for extension in [".cgf", ".cga", ".skin"] {
        let candidates: Vec<&Entry> = archive
            .entries()
            .iter()
            .filter(|entry| entry.path().to_ascii_lowercase().ends_with(extension))
            .collect();
        let stride = (candidates.len() / SAMPLE_PER_EXTENSION).max(1);
        sample.extend(
            candidates
                .into_iter()
                .step_by(stride)
                .take(SAMPLE_PER_EXTENSION),
        );
    }

    let results: Vec<Result<(usize, usize, usize), String>> = sample
        .par_iter()
        .enumerate()
        .map(|(position, entry)| {
            let glb = out.path().join(format!("{position}.glb"));
            let result = export(&index, entry, &glb);
            let _ignored = fs::remove_file(&glb);
            result.map_err(|error| format!("{}: {error}", entry.path()))
        })
        .collect();

    let failures: Vec<&String> = results.iter().filter_map(|r| r.as_ref().err()).collect();
    assert!(failures.is_empty(), "failed: {failures:#?}");
    let (textured, images, fallbacks) =
        results.iter().flatten().fold((0, 0, 0), |sum, (a, b, c)| {
            (sum.0 + a, sum.1 + b, sum.2 + c)
        });
    assert!(
        textured > 100,
        "only {textured} textured materials in the sample"
    );
    assert!(images > 100, "only {images} images in the sample");
    #[expect(clippy::cast_precision_loss, reason = "the counts are small")]
    let missing_share = fallbacks as f64 / (images + fallbacks).max(1) as f64;
    assert!(
        missing_share < 0.2,
        "{fallbacks} of {} textures could not be used",
        images + fallbacks
    );
    Ok(())
}

fn export(
    index: &PathIndex<'_>,
    entry: &Entry,
    glb: &Path,
) -> Result<(usize, usize, usize), String> {
    let Ok(source) = ModelSource::from_index(index, entry.path()) else {
        return Ok((0, 0, 0));
    };
    let options = GlbOptions::default()
        .with_materials(MaterialMode::Textures)
        .with_texture_mip(TEXTURE_MIP);
    let summary = source
        .write_glb_textured(index, &options, glb)
        .map_err(|error| format!("{error:#?}"))?;
    let bytes = fs::read(glb).map_err(|error| error.to_string())?;
    let checked = check_glb(&bytes)?;
    if checked.document.images().len() != summary.textures {
        return Err("the summary and the document disagree on the images".to_owned());
    }
    if checked.document.materials().len() != summary.materials {
        return Err("the summary and the document disagree on the materials".to_owned());
    }
    Ok((
        summary.textured_materials,
        summary.textures,
        summary.texture_fallbacks.len(),
    ))
}

fn entry_path(file: &str) -> String {
    let unified = file.replace('/', "\\");
    let stem = unified
        .rsplit_once('.')
        .map_or(unified.as_str(), |(stem, _)| stem);
    format!("Data\\{stem}.dds")
}

fn mip_of(texture: &Texture<'_>) -> u32 {
    let width = texture.header().width();
    (0..texture.header().mip_count())
        .find(|level| (width >> level) <= NORMAL_MAP_SIDE)
        .unwrap_or_else(|| texture.header().mip_count().saturating_sub(1))
}

fn correlation(a: &[f64], b: &[f64]) -> f64 {
    #[expect(clippy::cast_precision_loss, reason = "the counts are small")]
    let count = a.len() as f64;
    let (mean_a, mean_b) = (a.iter().sum::<f64>() / count, b.iter().sum::<f64>() / count);
    let (mut both, mut left, mut right) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        both = (x - mean_a).mul_add(y - mean_b, both);
        left += (x - mean_a).powi(2);
        right += (y - mean_b).powi(2);
    }
    both / (left.sqrt() * right.sqrt()).max(f64::MIN_POSITIVE)
}

/// Returns the correlation of the change of the red channel down the rows with the change of the
/// green channel along the columns. A normal map of a height field has gradients `(-h_x, ±h_y)`:
/// with green pointing up the image the two are opposite and the correlation is negative, with
/// green pointing down they agree and it is positive.
fn curl(image: &RgbaImage) -> Option<f64> {
    let (width, height) = (image.width() as usize, image.height() as usize);
    if width < 16 || height < 16 {
        return None;
    }
    let channel = |x: usize, y: usize, channel: usize| {
        let at = (y * width + x) * 4 + channel;
        (f64::from(image.pixels().get(at).copied().unwrap_or_default()) / 255.0).mul_add(2.0, -1.0)
    };
    let (mut red_down, mut green_along) = (Vec::new(), Vec::new());
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            red_down.push(channel(x, y + 1, 0) - channel(x, y - 1, 0));
            green_along.push(channel(x + 1, y, 1) - channel(x - 1, y, 1));
        }
    }
    Some(correlation(&red_down, &green_along))
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_normal_maps_of_the_engine_have_green_pointing_down_the_image() -> TestResult {
    let archive = Archive::open_with_key(
        real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let index = PathIndex::new(&archive);
    let files: BTreeSet<String> = archive
        .entries()
        .par_iter()
        .filter(|entry| entry.path().to_ascii_lowercase().ends_with(".mtl"))
        .flat_map(|entry| {
            let mut bytes = Vec::new();
            if let Ok(mut reader) = archive.open_entry(entry) {
                let _ignored = reader.read_to_end(&mut bytes);
            }
            MaterialLibrary::parse(entry.path(), &bytes).map_or_default(|library| {
                library
                    .materials
                    .into_iter()
                    .flat_map(|material| material.textures)
                    .map(|texture| texture.file)
                    .filter(|file| file.to_ascii_lowercase().ends_with("_ddna.tif"))
                    .collect::<Vec<_>>()
            })
        })
        .collect();

    let curls: Vec<f64> = files
        .iter()
        .step_by(NORMAL_MAP_SAMPLE_STRIDE)
        .collect::<Vec<_>>()
        .par_iter()
        .filter_map(|file| {
            let texture = Texture::from_index(&index, &entry_path(file)).ok()?;
            curl(&texture.decode_mip(mip_of(&texture)).ok()?)
        })
        .filter(|curl| curl.abs() > MIN_INTEGRABLE)
        .collect();

    assert!(
        curls.len() > MIN_NORMAL_MAPS,
        "only {} normal maps are integrable height fields",
        curls.len()
    );
    let down = curls.iter().filter(|curl| **curl > 0.0).count();
    #[expect(clippy::cast_precision_loss, reason = "the counts are small")]
    let share = down as f64 / curls.len() as f64;
    assert!(
        share >= MIN_DIRECTX_SHARE,
        "{down} of {} normal maps have green pointing down the image",
        curls.len()
    );
    Ok(())
}
