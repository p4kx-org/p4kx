// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Material libraries of a real Archive; runs with `--ignored` when `P4KX_DATA_P4K` names one.

use std::error::Error;
use std::fs;
use std::io::Read;
use std::path::Path;

use p4kx_extraction::model::{MaterialLibrary, MaterialStatus};
use p4kx_extraction::{Archive, Entry, GlbOptions, ModelSource, PathIndex};
use p4kx_testkit::{check_glb, real_archive_key, real_archive_path};
use rayon::prelude::*;

type TestResult = Result<(), Box<dyn Error>>;

const STATIC_EXTENSIONS: [&str; 2] = [".cgf", ".cga"];
const SKIN_EXTENSION: &str = ".skin";
const SAMPLE_PER_EXTENSION: usize = 150;
const MIN_STATIC_LOADED_SHARE: f64 = 0.9;

/// What one sampled model came to.
struct Outcome {
    is_static: bool,
    named: bool,
    loaded: bool,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn every_mtl_file_of_the_archive_is_read() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let mtl: Vec<&Entry> = archive
        .entries()
        .iter()
        .filter(|entry| entry.path().to_ascii_lowercase().ends_with(".mtl"))
        .collect();

    let failures: Vec<String> = mtl
        .par_iter()
        .filter_map(|entry| {
            let mut bytes = Vec::new();
            let read = archive
                .open_entry(entry)
                .map_err(|error| error.to_string())
                .and_then(|mut reader| {
                    reader
                        .read_to_end(&mut bytes)
                        .map_err(|error| error.to_string())
                });
            if let Err(error) = read {
                return Some(format!("{}: {error}", entry.path()));
            }
            MaterialLibrary::parse(entry.path(), &bytes)
                .err()
                .map(|error| format!("{}: {error}", entry.path()))
        })
        .collect();

    assert!(!mtl.is_empty(), "the Archive holds .mtl files");
    assert!(failures.is_empty(), "failed: {failures:#?}");
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_sample_of_models_finds_its_material_library_and_exports_valid_glb() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let index = PathIndex::new(&archive);
    let out = tempfile::tempdir()?;
    let mut sample: Vec<&Entry> = Vec::new();
    for extension in [STATIC_EXTENSIONS[0], STATIC_EXTENSIONS[1], SKIN_EXTENSION] {
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

    let results: Vec<Result<Outcome, String>> = sample
        .par_iter()
        .enumerate()
        .map(|(position, entry)| {
            let glb = out.path().join(format!("{position}.glb"));
            let result = check(&index, entry, &glb);
            let _ignored = fs::remove_file(&glb);
            result.map_err(|error| format!("{}: {error}", entry.path()))
        })
        .collect();

    let failures: Vec<&String> = results.iter().filter_map(|r| r.as_ref().err()).collect();
    assert!(failures.is_empty(), "failed: {failures:#?}");
    let statics: Vec<&Outcome> = results
        .iter()
        .flatten()
        .filter(|outcome| outcome.is_static && outcome.named)
        .collect();
    let loaded = statics.iter().filter(|outcome| outcome.loaded).count();
    assert!(!statics.is_empty(), "the sample names material libraries");
    #[expect(clippy::cast_precision_loss, reason = "the counts are small")]
    let share = loaded as f64 / statics.len() as f64;
    assert!(
        share >= MIN_STATIC_LOADED_SHARE,
        "only {loaded} of {} libraries of static models were read",
        statics.len()
    );
    Ok(())
}

fn check(index: &PathIndex<'_>, entry: &Entry, glb: &Path) -> Result<Outcome, String> {
    let is_static = !entry.path().to_ascii_lowercase().ends_with(SKIN_EXTENSION);
    let source =
        ModelSource::from_index(index, entry.path()).map_err(|error| format!("{error:#?}"))?;
    let summary = source
        .write_glb(&GlbOptions::default(), glb)
        .map_err(|e| format!("{e:#?}"))?;
    let bytes = fs::read(glb).map_err(|e| e.to_string())?;
    let checked = check_glb(&bytes)?;
    let model = source.model();
    if checked.document.materials().len() != summary.materials {
        return Err("the summary and the document disagree on the materials".to_owned());
    }
    if let MaterialStatus::Unreadable { reason, .. } = &model.materials {
        return Err(format!("unreadable material library: {reason}"));
    }
    Ok(Outcome {
        is_static,
        named: model.material_library.is_some(),
        loaded: model.materials.library().is_some(),
    })
}
