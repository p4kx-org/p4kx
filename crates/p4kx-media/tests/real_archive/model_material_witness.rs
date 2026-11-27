// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::Path;

use p4kx_extraction::{Archive, ArchiveWatch, Filter};
use p4kx_media::{ExportOptions, ExportOutcome, Identity, MediaKind, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::{Value, json};

use super::TestResult;
use super::archive_replay::Lines;
use super::shader_accounting::require;

#[path = "model_material_witness_origins.rs"]
mod origins;
#[path = "model_material_witness_paths.rs"]
mod paths;
#[path = "model_material_witness_selection.rs"]
mod selection;

const ENTRY_BYTES: u64 = 512 << 10;

#[derive(Debug)]
enum InstalledFailure {
    Configuration,
    Sample {
        ordinal: usize,
        stage: InstalledStage,
        counts: selection::ProbeCounts,
        interrupted: bool,
    },
    Channel {
        ordinal: usize,
        stage: InstalledStage,
    },
}

#[derive(Clone, Copy, Debug)]
enum InstalledStage {
    Open,
    InitialWatch,
    PackedSelection,
    PackedReplay,
    CdfSelection,
    CdfReplay,
    FinalWatch,
    WriteSummary,
}

fn stage<T, E>(
    outcome: Result<T, E>,
    ordinal: usize,
    stage: InstalledStage,
) -> Result<T, InstalledFailure> {
    outcome.map_err(|_| InstalledFailure::Channel { ordinal, stage })
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_model_material_references_match_original_paths_and_field_origins() -> Result<(), String>
{
    verify().map_err(|failure| match failure {
        InstalledFailure::Configuration => {
            "model material Archive configuration unavailable".into()
        }
        InstalledFailure::Sample {
            ordinal,
            stage,
            counts,
            interrupted,
        } => {
            format!(
                "model material Archive {ordinal}: {stage:?}, interrupted={interrupted}, {counts:?}"
            )
        }
        InstalledFailure::Channel { ordinal, stage } => {
            format!("model material Archive {ordinal}: {stage:?}")
        }
    })
}

#[test]
fn synthetic_archive_paths_and_field_origins_reconcile_through_the_same_witness() -> TestResult {
    use p4kx_extraction::chunks::ivo;
    use p4kx_testkit::TestChunkFile;

    let folder = tempfile::tempdir()?;
    let path = folder.path().join("synthetic.p4k");
    let model = TestChunkFile::ivo()
        .chunk(ivo::MTL_NAME_320.id(), 0x900, b"surface.mtl\0")
        .bytes();
    let cdf = br"<CharacterDefinition><AttachmentList><Attachment Binding='Models/Hull.cga' Material='surface.mtl'/></AttachmentList></CharacterDefinition>";
    let qualified = br"<CharacterDefinition><AttachmentList><Attachment Material='\Shared\Paint.MTL'/></AttachmentList></CharacterDefinition>";
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Models/Hull.cga", model))
        .entry(TestEntry::zstd("Data/Definitions/Ship.cdf", cdf).encrypted())
        .entry(TestEntry::stored("Data/Definitions/Native.cdf", qualified))
        .entry(TestEntry::stored(
            "Data/Shared/Paint.mtl",
            b"qualified target",
        ))
        .entry(TestEntry::stored(
            "Data/Models/surface.mtl",
            b"unread installed target",
        ))
        .entry(TestEntry::stored(
            "Data/Definitions/surface.mtl",
            b"wrong directory",
        ))
        .entry(TestEntry::stored(
            "Data/surface.mtl",
            b"losing Data fallback",
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let watch = ArchiveWatch::new(&archive);
    verify_archive(&archive, &watch)?;
    let original = selection::Original::new(&archive, &watch);
    let mut counts = selection::ProbeCounts::default();
    let independent = original
        .select_matching(selection::Family::Cdf, &mut counts, |entry| {
            entry.path().ends_with("Native.cdf")
        })?
        .ok_or("positive binding-independent CDF")?;
    require(
        independent.binding.is_none(),
        "qualified path has no synthetic model binding",
    )?;
    replay(&independent, selection::Family::Cdf)?;
    drop(archive);
    folder.close()?;
    Ok(())
}

fn verify() -> Result<(), InstalledFailure> {
    let primary = p4kx_testkit::real_archive_path().map_err(|_| InstalledFailure::Configuration)?;
    let secondary = p4kx_testkit::real_archive_b_path();
    let key = p4kx_testkit::real_archive_key().ok();
    for (channel, path) in std::iter::once(primary).chain(secondary).enumerate() {
        let ordinal = channel + 1;
        let archive = stage(
            Archive::open_with_key(path, key.clone()),
            ordinal,
            InstalledStage::Open,
        )?;
        let watch = ArchiveWatch::new(&archive);
        stage(watch.check(), ordinal, InstalledStage::InitialWatch)?;
        let original = selection::Original::new(&archive, &watch);
        for (family, select, check) in [
            (
                selection::Family::Packed,
                InstalledStage::PackedSelection,
                InstalledStage::PackedReplay,
            ),
            (
                selection::Family::Cdf,
                InstalledStage::CdfSelection,
                InstalledStage::CdfReplay,
            ),
        ] {
            let mut counts = selection::ProbeCounts::default();
            let sample = original.select(family, &mut counts);
            let interrupted = sample.is_err();
            let sample = sample.ok().flatten().ok_or(InstalledFailure::Sample {
                ordinal,
                stage: select,
                counts,
                interrupted,
            })?;
            stage(replay(&sample, family), ordinal, check)?;
        }
        stage(watch.check(), ordinal, InstalledStage::FinalWatch)?;
        stage(
            writeln!(
                io::stdout().lock(),
                "Model material Archive {ordinal}: 1 packed, 1 CDF, 2 verified material targets",
            ),
            ordinal,
            InstalledStage::WriteSummary,
        )?;
    }
    Ok(())
}

fn verify_archive(archive: &Archive, watch: &ArchiveWatch) -> TestResult {
    let original = selection::Original::new(archive, watch);
    for family in [selection::Family::Packed, selection::Family::Cdf] {
        let mut counts = selection::ProbeCounts::default();
        let sample = original
            .select(family, &mut counts)?
            .ok_or("positive fixture reference")?;
        replay(&sample, family)?;
    }
    Ok(())
}

fn directory_replay(path: &Path, sample: &selection::Sample<'_>) -> TestResult<Archive> {
    let source = if sample.entry.is_encrypted() {
        TestEntry::zstd(sample.entry.path(), sample.bytes.as_slice()).encrypted()
    } else {
        TestEntry::stored(sample.entry.path(), sample.bytes.as_slice())
    };
    let mut builder = ArchiveBuilder::new().entry(source);
    for target in sample.targets() {
        let entry = TestEntry::stored(target.path(), b"unread placeholder").with_crc32c(0);
        builder = builder.entry(if target.is_encrypted() {
            entry.flagged_encrypted()
        } else {
            entry
        });
    }
    builder.write_to(path)?;
    let replay = Archive::open_with_key(path, Some(test_key()))?;
    prove_directory(&replay, sample)?;
    Ok(replay)
}

fn replay(sample: &selection::Sample<'_>, family: selection::Family) -> TestResult {
    let work = tempfile::tempdir()?;
    let replay = directory_replay(&work.path().join("replay.p4k"), sample)?;
    let include = Filter::parse(sample.entry.path())?;
    require(
        replay
            .entries()
            .iter()
            .filter(|entry| include.matches(entry.path()))
            .count()
            == 1,
        "Replay selects exactly the verified original source",
    )?;
    let output = work.path().join("output");
    let mut options = ExportOptions {
        kinds: vec![MediaKind::Model],
        include: Some(include),
        ..ExportOptions::default()
    };
    options.limits.entry_bytes = ENTRY_BYTES;
    let ExportOutcome::Published { index, .. } = p4kx_media::export(&replay, &output, &options)?
    else {
        return Err("bounded model-material replay did not publish".into());
    };
    require(
        index.source_entries.written == 1,
        "target payloads were not read",
    )?;
    let base = output.join(&index.artifact_base);
    let (ordinal, row) = select_row(Lines::open(&base.join("model.jsonl"))?, sample, family)?;
    let data = row
        .data
        .pointer(family.reference())
        .ok_or("projected material reference")?;
    compare_reference(data, sample.material, 1, &sample.authored)?;
    if let Some(binding) = &sample.binding {
        let data = row
            .data
            .pointer("/payload/binding")
            .ok_or("projected CDF binding")?;
        compare_reference(data, binding.target, 2, &binding.authored)?;
    }
    let raw = row
        .source
        .raw
        .as_ref()
        .ok_or("verified Model or CDF raw artifact")?;
    let retained =
        super::manifests::check_payload(&base, &raw.path, raw.size, &raw.digest, ENTRY_BYTES)?;
    require(
        retained == sample.bytes,
        "retained artifact differs from independently verified source bytes",
    )?;
    origins::verify(&base, ordinal, sample, family)?;
    drop(replay);
    work.close()?;
    Ok(())
}

fn prove_directory(replay: &Archive, sample: &selection::Sample<'_>) -> TestResult {
    let original = std::iter::once(sample.entry).chain(sample.targets());
    require(
        replay.entries().len() == original.clone().count(),
        "directory witness size",
    )?;
    for (current, source) in replay.entries().iter().zip(original) {
        require(
            current.path() == source.path() && current.is_encrypted() == source.is_encrypted(),
            "replay identity must preserve original path spelling and encryption",
        )?;
    }
    Ok(())
}

fn compare_reference(
    value: &Value,
    original: &p4kx_extraction::Entry,
    replay_index: usize,
    authored: &str,
) -> TestResult {
    for (pointer, expected) in [
        ("/original/value", json!(authored)),
        ("/resolution/status", json!("not-selected")),
        ("/resolution/reason", json!("filter")),
        (
            "/resolution/target/source/entry/path",
            json!(original.path()),
        ),
        (
            "/resolution/target/source/entry/entry_index",
            json!(replay_index),
        ),
        ("/resolution/target/source/containers", json!([])),
    ] {
        require(
            value.pointer(pointer) == Some(&expected),
            "physical reference differs from original",
        )?;
    }
    Ok(())
}

fn select_row(
    mut rows: Lines,
    sample: &selection::Sample<'_>,
    family: selection::Family,
) -> TestResult<(usize, MediaRecord)> {
    let mut ordinal = 0;
    let mut matched = None;
    while let Some(row) = rows.next::<MediaRecord>()? {
        if row.subresource.namespace == family.namespace()
            && row.subresource.indices == sample.indices
        {
            require(matched.is_none(), "duplicate source occurrence")?;
            require(
                matches!(
                    &row.source.identity,
                    Identity::Entry { entry, containers }
                    if entry.entry_index == 0 && entry.path == sample.entry.path() && containers.is_empty()
                ),
                "source identity did not join the replay directory",
            )?;
            if let Some(range) = &sample.range {
                require(
                    row.data.pointer("/payload/token")
                        == Some(
                            &json!({"basis":{"type":"entry"}, "start":range.start, "end_exclusive":range.end}),
                        ),
                    "packed token extent differs from independently observed bytes",
                )?;
                require(
                    row.data.pointer("/payload/primary_model_reader_selector")
                        == Some(&json!(true)),
                    "selected packed declaration is not the primary measured name",
                )?;
            }
            matched = Some((ordinal, row));
        }
        ordinal += 1;
    }
    matched.ok_or_else(|| "no projected source occurrence".into())
}
