// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, File, OpenOptions};
use std::io::Write;

use p4kx_extraction::{Archive, ArchiveWatch, ArchiveWriteProtection, ConflictPolicy};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::checkpoint::Checkpoint;
use crate::error;
use crate::export_run;
use crate::generation::{Boundary, Generation};
use crate::staged_directory::StagedDirectory;
use crate::{ExportOptions, Limits, MediaError, ProviderRegistry};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn failure_before_generation_construction_releases_the_kernel_lock() -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = folder.path().join("Data.p4k");
    ArchiveBuilder::new().write_to(&source)?;
    let archive = Archive::open(source)?;
    let options = ExportOptions::default();
    let watch = ArchiveWatch::new(&archive);
    let protection = ArchiveWriteProtection::new(archive.path(), options.cancel.flag())?;
    let checkpoint = Checkpoint {
        cancel: &options.cancel,
        watch: &watch,
        protection: &protection,
    };
    let output = folder.path().join("media");
    let result = Generation::with_stage(
        &output,
        Limits::default(),
        &checkpoint,
        |output, checkpoint| {
            let path = output.join(".media.lock");
            File::create_new(&path).map_err(|source| error::io(&path, source))?;
            StagedDirectory::create(output, ".media-stage-", checkpoint)
        },
    );
    assert!(
        result.is_err(),
        "injected real create failure occurs after locking but before construction"
    );
    let generation = Generation::new(&output, Limits::default(), &checkpoint)?;
    generation.finish(Ok(()))?;
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
    let protection = ArchiveWriteProtection::new(new_archive.path(), options.cancel.flag())?;
    let checkpoint = Checkpoint {
        cancel: &options.cancel,
        watch: &watch,
        protection: &protection,
    };
    let registry = ProviderRegistry::default();
    let providers = registry.selected(&[])?;
    let mut generation = Generation::new(&output, Limits::default(), &checkpoint)?;
    let mut index = export_run::build(
        &new_archive,
        generation.stage.permit(),
        &options,
        &providers,
        Checkpoint {
            cancel: &options.cancel,
            watch: &watch,
            protection: &protection,
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
    generation.finish(Ok(()))?;
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

#[test]
fn cancellation_at_index_replacement_cleans_the_temporary_index_and_keeps_the_commit() -> TestResult
{
    let folder = tempfile::tempdir()?;
    let archive = named_archive(folder.path(), "old-cancel")?;
    let output = folder.path().join("media");
    crate::export(&archive, &output, &ExportOptions::default())?;
    let original = fs::read(output.join("media-index.json"))?;
    let next = named_archive(folder.path(), "next-cancel")?;
    let options = ExportOptions::default();
    let watch = ArchiveWatch::new(&next);
    let protection = ArchiveWriteProtection::new(next.path(), options.cancel.flag())?;
    let checkpoint = Checkpoint {
        cancel: &options.cancel,
        watch: &watch,
        protection: &protection,
    };
    let registry = ProviderRegistry::default();
    let providers = registry.selected(&[])?;
    let mut generation = Generation::new(&output, Limits::default(), &checkpoint)?;
    let mut index = export_run::build(
        &next,
        generation.stage.permit(),
        &options,
        &providers,
        checkpoint,
    )?;
    let result = generation.publish_observed(
        &mut index,
        ConflictPolicy::Overwrite,
        &checkpoint,
        |boundary| {
            if boundary == Boundary::BeforeIndexReplace {
                options.cancel.cancel();
            }
            Ok(())
        },
    );
    assert!(matches!(
        generation.finish(result),
        Err(MediaError::Cancelled)
    ));
    assert_eq!(fs::read(output.join("media-index.json"))?, original);
    let leftovers = fs::read_dir(&output)?.collect::<Result<Vec<_>, _>>()?;
    let expected = [".media.lock", "generations", "media-index.json"];
    assert_eq!(leftovers.len(), expected.len());
    assert!(expected.iter().all(|name| output.join(name).exists()));
    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum ReuseStop {
    Cancelled,
    SourceChanged,
}

#[test]
fn unchanged_generation_checks_cancel_and_source_at_its_return_boundary() -> TestResult {
    for stop in [ReuseStop::Cancelled, ReuseStop::SourceChanged] {
        unchanged_generation_stop(stop)?;
    }
    Ok(())
}

fn unchanged_generation_stop(stop: ReuseStop) -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = named_archive(folder.path(), "unchanged")?;
    let output = folder.path().join("media");
    let options = ExportOptions::default();
    crate::export(&archive, &output, &options)?;
    let previous_index = fs::read(output.join("media-index.json"))?;

    let watch = ArchiveWatch::new(&archive);
    let protection = ArchiveWriteProtection::new(archive.path(), options.cancel.flag())?;
    let checkpoint = Checkpoint {
        cancel: &options.cancel,
        watch: &watch,
        protection: &protection,
    };
    let registry = ProviderRegistry::default();
    let providers = registry.selected(&[])?;
    let mut generation = Generation::new(&output, Limits::default(), &checkpoint)?;
    let mut index = export_run::build(
        &archive,
        generation.stage.permit(),
        &options,
        &providers,
        checkpoint,
    )?;
    assert_eq!(
        generation
            .previous
            .as_ref()
            .map(|previous| &previous.generation),
        Some(&index.generation),
        "unchanged data takes the same-generation branch"
    );

    let result = generation.publish_observed(
        &mut index,
        ConflictPolicy::IfChanged,
        &checkpoint,
        |boundary| {
            if boundary == Boundary::ArtifactsComplete {
                stop.inject(&archive, &options)?;
            }
            Ok(())
        },
    );
    let result = generation.finish(result);
    assert!(
        matches!(
            (stop, result),
            (ReuseStop::Cancelled, Err(MediaError::Cancelled))
                | (ReuseStop::SourceChanged, Err(MediaError::Changed(_)))
        ),
        "{stop:?} must not return a successful reuse"
    );
    assert_eq!(
        fs::read(output.join("media-index.json"))?,
        previous_index,
        "{stop:?} leaves the verified previous index in place"
    );
    Ok(())
}

impl ReuseStop {
    fn inject(self, archive: &Archive, options: &ExportOptions) -> Result<(), MediaError> {
        match self {
            Self::Cancelled => {
                options.cancel.cancel();
                Ok(())
            }
            Self::SourceChanged => {
                let path = archive.path();
                OpenOptions::new()
                    .append(true)
                    .open(path)
                    .and_then(|mut file| file.write_all(b"changed"))
                    .map_err(|source| error::io(path, source))
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum HashFailureCase {
    Limit,
    Cancelled,
}

#[test]
fn generation_hash_failures_restore_both_original_index_fields() -> TestResult {
    for case in [HashFailureCase::Limit, HashFailureCase::Cancelled] {
        let folder = tempfile::tempdir()?;
        let archive = named_archive(folder.path(), "hash")?;
        let output = folder.path().join("media");
        let options = ExportOptions::default();
        crate::export(&archive, &output, &options)?;
        let mut index: crate::Index =
            serde_json::from_slice(&fs::read(output.join("media-index.json"))?)?;
        let generation = index.generation.clone();
        let artifact_base = index.artifact_base.clone();
        let watch = ArchiveWatch::new(&archive);
        let protection = ArchiveWriteProtection::new(archive.path(), options.cancel.flag())?;
        let checkpoint = Checkpoint {
            cancel: &options.cancel,
            watch: &watch,
            protection: &protection,
        };
        match case {
            HashFailureCase::Limit => index.limits.record_bytes = 1,
            HashFailureCase::Cancelled => options.cancel.cancel(),
        }
        let failure = crate::hashing::generation(&mut index, &checkpoint)
            .expect_err("generation hashing must retain the actual failure");
        assert_eq!(index.generation, generation, "{case:?} restores generation");
        assert_eq!(
            index.artifact_base, artifact_base,
            "{case:?} restores artifact base"
        );
        match case {
            HashFailureCase::Limit => assert!(
                matches!(
                    failure,
                    MediaError::Limit {
                        budget: "index_bytes",
                        limit: 1
                    }
                ),
                "hashing retains the original configured index limit: {failure:?}"
            ),
            HashFailureCase::Cancelled => assert!(
                matches!(failure, MediaError::Cancelled),
                "one actual guarded cancellation stays one typed stop: {failure:?}"
            ),
        }
    }
    Ok(())
}
