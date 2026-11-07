// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, File};

use p4kx_extraction::{Archive, ArchiveWatch, ConflictPolicy};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::checkpoint::Checkpoint;
use crate::error;
use crate::export_run;
use crate::generation::{Boundary, Generation};
use crate::{ExportOptions, Limits, ProviderRegistry};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn failure_before_generation_construction_releases_the_kernel_lock() -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = folder.path().join("Data.p4k");
    ArchiveBuilder::new().write_to(&source)?;
    let archive = Archive::open(source)?;
    let options = ExportOptions::default();
    let watch = ArchiveWatch::new(&archive);
    let checkpoint = Checkpoint {
        cancel: &options.cancel,
        watch: &watch,
    };
    let output = folder.path().join("media");
    let result = Generation::with_stage(&output, Limits::default(), &checkpoint, |output| {
        File::create_new(output.join(".media.lock"))?;
        tempfile::tempdir_in(output)
    });
    assert!(
        result.is_err(),
        "injected real create failure occurs after locking but before construction"
    );
    let generation = Generation::new(&output, Limits::default(), &checkpoint)?;
    generation.release()?;
    Ok(())
}

#[test]
fn write_failures_at_actual_artifact_and_index_boundaries_preserve_old_commit() -> TestResult {
    for boundary in [
        Boundary::ArtifactsComplete,
        Boundary::GenerationPlaced,
        Boundary::IndexSynced,
        Boundary::BeforeIndexReplace,
    ] {
        fails_at(boundary)?;
    }
    Ok(())
}

fn fails_at(boundary: Boundary) -> TestResult {
    let folder = tempfile::tempdir()?;
    let old_archive = named_archive(folder.path(), "old")?;
    let output = folder.path().join("media");
    crate::export(&old_archive, &output, &ExportOptions::default())?;
    let old_index = fs::read(output.join("media-index.json"))?;
    let new_archive = named_archive(folder.path(), "new")?;
    let options = ExportOptions::default();
    let watch = ArchiveWatch::new(&new_archive);
    let checkpoint = Checkpoint {
        cancel: &options.cancel,
        watch: &watch,
    };
    let registry = ProviderRegistry::default();
    let providers = registry.selected(&[])?;
    let generation = Generation::new(&output, Limits::default(), &checkpoint)?;
    let mut index = export_run::build(
        &new_archive,
        generation.stage.path(),
        &options,
        &providers,
        Checkpoint {
            cancel: &options.cancel,
            watch: &watch,
        },
    )?;
    let denied = output.join("media-index.json");
    let result = generation.publish_observed(
        &mut index,
        ConflictPolicy::Overwrite,
        &checkpoint,
        |actual| {
            if actual == boundary {
                File::create_new(&denied).map_err(|source| error::io(&denied, source))?;
            }
            Ok(())
        },
    );
    assert!(
        result.is_err(),
        "real filesystem write failure at {boundary:?} must abort commit"
    );
    assert_eq!(
        fs::read(&denied)?,
        old_index,
        "{boundary:?} preserved the prior complete root index"
    );
    generation.release()?;
    let outcome = crate::export(&old_archive, &output, &ExportOptions::default())?;
    assert!(
        matches!(outcome, crate::ExportOutcome::Skipped { .. }),
        "prior referenced artifacts still validate"
    );
    Ok(())
}

fn named_archive(
    root: &std::path::Path,
    name: &str,
) -> Result<Archive, Box<dyn std::error::Error>> {
    let path = root.join(format!("{name}.p4k"));
    ArchiveBuilder::new()
        .entry(TestEntry::stored(name, name.as_bytes()))
        .write_to(&path)?;
    Ok(Archive::open(path)?)
}
