// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Extraction, `CryXml`, texture and sound Conversion on a real Archive; runs with `--ignored` when
//! `P4KX_DATA_P4K` names one.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::io::Read;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use p4kx_extraction::cryxml::{self, Format};
use p4kx_extraction::{Archive, ConflictPolicy, Entry, ExtractOptions, extract};
use p4kx_testkit::real_archive_path;
use rayon::prelude::*;

mod adpcm;
mod animations;
mod audio_survey;
mod audio_verification;
mod bench;
mod chunk_types;
mod chunks;
mod combined;
mod conversion;
mod coverage;
mod crch_models;
mod crch_survey;
mod cryxml_trees;
mod cubemap_alpha;
mod dataforge_canary;
mod dequantisation_boxes;
mod error_values;
mod field_statistics;
mod glb_hashes;
mod loc_keys;
mod localization;
mod materials;
mod mesh_streams;
mod model_details;
mod model_lods;
mod model_verification;
mod models;
mod portable_names;
mod records;
mod scmdb;
mod scmdb_compare;
mod second_uvs;
mod shader_census;
mod skeletons;
mod soc_chunks;
mod sound;
mod sound_extraction;
mod soundbank;
mod soundbank_hirc;
mod stencil_census;
mod stencil_params;
mod stream_repeats;
mod texture;
mod texture_conversion;
mod texture_cube_order;
mod texture_float;
mod texture_hdr;
mod texture_hdr_cube;
mod texture_known;
mod texture_survey;
mod texture_tails;
mod textured_materials;
mod ui;
mod ui_fonts;
mod ui_render;
mod ui_sample;
mod verification;
mod verify;
mod vertex_attributes;
mod vertex_colours;
mod vorbis_cost;
mod weak_pointers;
mod wem_survey;
mod without_materials;

type TestResult = Result<(), Box<dyn Error>>;
type SendError = Box<dyn Error + Send + Sync>;

#[expect(
    clippy::print_stdout,
    reason = "the counts and timings of a real Archive are what the verification reports"
)]
fn report(line: &str) {
    println!("{line}");
}

const PER_KIND: usize = 10;
const MAX_SIZE: u64 = 4 << 20;
const CRYXML_EXTENSIONS: [&str; 2] = [".xml", ".mtl"];
const MAX_CRYXML_SIZE: u64 = 64 << 20;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn converts_every_cryxml_entry_of_a_real_archive() -> TestResult {
    let archive = Archive::open_with_key(
        real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let candidates: Vec<&Entry> = archive
        .entries()
        .iter()
        .filter(|entry| entry.uncompressed_size() <= MAX_CRYXML_SIZE)
        .filter(|entry| {
            let path = entry.path().to_ascii_lowercase();
            CRYXML_EXTENSIONS
                .iter()
                .any(|extension| path.ends_with(extension))
        })
        .collect();
    let binary = AtomicUsize::new(0);
    let text = AtomicUsize::new(0);
    let started = Instant::now();

    let failures: Vec<String> = candidates
        .par_iter()
        .filter_map(|entry| match convert(&archive, entry) {
            Ok(Some(Format::CryXml(_))) => {
                binary.fetch_add(1, Ordering::Relaxed);
                None
            }
            Ok(Some(Format::Text)) => {
                text.fetch_add(1, Ordering::Relaxed);
                None
            }
            Ok(None) => None,
            Err(error) => Some(format!("{}: {error}", entry.path())),
        })
        .collect();

    assert!(
        failures.is_empty(),
        "{} of {} Entries fail, e.g. {:?}",
        failures.len(),
        candidates.len(),
        failures.iter().take(10).collect::<Vec<_>>()
    );
    let (binary, text) = (binary.into_inner(), text.into_inner());
    report(&format!(
        "{} candidates: {binary} CryXml converted, {text} text passed through, in {:?}",
        candidates.len(),
        started.elapsed()
    ));
    assert!(
        binary > 1000,
        "converted {binary} CryXml and passed {text} text Entries through"
    );
    Ok(())
}

fn convert(archive: &Archive, entry: &Entry) -> Result<Option<Format>, SendError> {
    let mut bytes = Vec::with_capacity(usize::try_from(entry.uncompressed_size())?);
    archive.open_entry(entry)?.read_to_end(&mut bytes)?;
    let Some(format) = cryxml::detect(&bytes) else {
        return Ok(None);
    };
    let mut out = Vec::with_capacity(bytes.len() * 2);
    cryxml::read(&bytes)?.write_to(&mut out)?;
    if format == Format::Text {
        if out != bytes {
            return Err("text XML changed".into());
        }
    } else {
        let xml = std::str::from_utf8(&out)?;
        if xml.contains('\r') {
            return Err("XML has a CR".into());
        }
        roxmltree::Document::parse(xml)?;
    }
    Ok(Some(format))
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn extracts_a_small_subset_of_a_real_archive() -> TestResult {
    let archive = Archive::open_with_key(
        real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let subset = small_subset(archive.entries());
    let target = tempfile::tempdir()?;

    let report = extract(&archive, &subset, target.path(), &ExtractOptions::default());

    assert!(
        report.is_complete(),
        "every Entry is written: {:?}",
        report.failures()
    );
    assert_eq!(
        report.extracted(),
        subset.len(),
        "the whole subset is written"
    );
    for entry in &subset {
        let file = target.path().join(entry.path().replace('\\', "/"));
        assert_eq!(
            std::fs::metadata(&file)?.len(),
            entry.uncompressed_size(),
            "{} has its uncompressed size on disk",
            entry.path()
        );
    }

    let options = ExtractOptions {
        policy: ConflictPolicy::IfChanged,
        ..ExtractOptions::default()
    };
    let again = extract(&archive, &subset, target.path(), &options);

    assert_eq!(
        again.skipped().len(),
        subset.len(),
        "unchanged files are kept"
    );
    Ok(())
}

fn small_subset(entries: &[Entry]) -> Vec<&Entry> {
    let mut per_kind = HashMap::new();
    let mut seen = HashSet::new();
    entries
        .iter()
        .filter(|entry| entry.uncompressed_size() <= MAX_SIZE)
        .filter(|entry| seen.insert(entry.path().replace('/', "\\").to_lowercase()))
        .filter(|entry| {
            let count = per_kind
                .entry((entry.compression(), entry.is_encrypted()))
                .or_insert(0);
            *count += 1;
            *count <= PER_KIND
        })
        .collect()
}
