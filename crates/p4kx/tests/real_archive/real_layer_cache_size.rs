// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The layer files the ship material libraries name: how many there are, and how much memory
//! their parsed libraries take against the bytes of the files (ADR-0301).

#![expect(clippy::print_stdout, reason = "the measurements are the output")]

use std::collections::HashSet;
use std::error::Error;

use p4kx_extraction::model::{LayerCache, LayerLimits, MAX_LAYER_FILES, MaterialStatus};
use p4kx_extraction::{Archive, ModelSource, PathIndex, TextureVerdicts};
use p4kx_testkit::{real_archive_key, real_archive_path};

use super::real_relief_decals::ship_libraries;

type TestResult = Result<(), Box<dyn Error>>;

/// The most the estimated parsed size of the layer files may be, as a multiple of their bytes.
const MAX_PARSED_FACTOR: f64 = 8.0;
const MIN_LAYER_FILES: usize = 20;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(clippy::cast_precision_loss, reason = "a ratio for a report")]
fn parsed_layer_libraries_stay_within_a_factor_of_their_file_bytes() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let index = PathIndex::new(&archive);
    let verdicts = TextureVerdicts::new();
    let cache = LayerCache::with_limits(LayerLimits {
        files_per_load: usize::MAX,
        bytes_per_load: usize::MAX,
        cached_files: usize::MAX,
        cached_bytes: usize::MAX,
    });
    let mut paths = HashSet::new();
    let mut per_library = 0;
    for (entry_path, library) in ship_libraries(&archive) {
        let status =
            ModelSource::material_library(&index, &entry_path, &library, &cache, &verdicts);
        let Some(library) = status.library() else {
            continue;
        };
        let mut own = HashSet::new();
        for layer in library
            .materials
            .iter()
            .flat_map(|material| &material.layers)
        {
            if let Some(path) = layer.attribute("Path").map(str::trim) {
                let normal = path.replace('\\', "/").to_ascii_lowercase();
                own.insert(normal.clone());
                paths.insert(normal);
            }
        }
        per_library = per_library.max(own.len());
    }
    let mut budget = cache.budget();
    let (mut file_bytes, mut parsed_bytes, mut read) = (0_usize, 0_usize, 0_usize);
    for path in &paths {
        let status = ModelSource::layer_library(&index, path, &cache, &verdicts, &mut budget);
        let MaterialStatus::Loaded(library) = &*status else {
            continue;
        };
        let entry = index
            .find(&format!("Data/{path}.mtl"))
            .or_else(|| index.find(&format!("Data/{path}")));
        file_bytes += entry.map_or(0, |entry| {
            usize::try_from(entry.uncompressed_size()).unwrap_or(0)
        });
        parsed_bytes += library.estimated_bytes();
        read += 1;
    }
    let factor = parsed_bytes as f64 / file_bytes.max(1) as f64;
    println!(
        "LAYERS: {read} of {} named files read, {file_bytes} file bytes, {parsed_bytes} estimated \
         parsed bytes (factor {factor:.2}); most layer files of one library: {per_library} \
         (per-load limit {MAX_LAYER_FILES})",
        paths.len()
    );
    assert!(read >= MIN_LAYER_FILES, "only {read} layer files");
    assert!(factor <= MAX_PARSED_FACTOR, "factor {factor:.2}");
    Ok(())
}
