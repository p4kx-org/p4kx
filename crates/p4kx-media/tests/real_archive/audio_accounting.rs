// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use p4kx_extraction::{Archive, ArchiveWatch, Entry, Exclusion, Filter, Soundbank, Wem};
use p4kx_media::{ExportOptions, ExportOutcome, Identity, Index, MediaKind, MediaRecord, Status};
use serde_json::{Value, json};

use super::shader_accounting::require;
use super::{TestResult, privacy};

#[path = "audio_bank_accounting.rs"]
mod bank;
#[path = "audio_container_accounting.rs"]
mod container;

#[path = "audio_loose_accounting.rs"]
mod loose;

const MAX_SAMPLE_BYTES: u64 = 8 << 20;
const MAX_PROBES_PER_KIND: usize = 32;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn original_audio_directories_and_retained_ranges_reconcile()
-> Result<(), privacy::VerificationFailure> {
    privacy::redact(verify_archives(verify_archive))
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn original_hirc_containers_and_children_reconcile() -> Result<(), privacy::VerificationFailure> {
    privacy::redact(verify_archives(verify_archive_containers))
}

#[test]
fn directory_candidates_leave_exact_selected_payload_accounting_intact()
-> Result<(), privacy::VerificationFailure> {
    privacy::redact(verify_synthetic_accounting())
}

fn verify_synthetic_accounting() -> TestResult {
    use p4kx_testkit::{ArchiveBuilder, TestCodec, TestEntry, TestWem};

    const SELECTED: &str = "Data/Sounds/wwise/Media/17.wem";
    const FILTERED: &str = "Data/Sounds/wwise/German/Media/17.wem";

    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let original = TestWem::pcm(TestCodec::Pcm, 1, 22_050, &[0, 1, 2]).bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(SELECTED, original.as_slice()))
        .entry(TestEntry::stored(FILTERED, b"filtered payload"))
        .entry(TestEntry::stored(FILTERED, b"duplicate filtered payload"))
        .entry(TestEntry::stored("excluded.socpak", b"unopened directory"))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    verify_sample(
        &archive,
        Sample {
            filter: Filter::parse(SELECTED)?,
            entry_index: 0,
            original,
        },
        &schema,
        verify_wem,
    )
}

#[test]
fn embedded_complete_prefetch_and_opaque_source_ranges_reconcile() -> TestResult {
    use p4kx_testkit::{ArchiveBuilder, TestBank, TestCodec, TestEntry, TestWem};

    const SELECTED: &str = "Data/Sounds/wwise/sample.bnk";

    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let complete = TestWem::pcm(TestCodec::Pcm, 1, 22_050, &[0, 1, 2]).bytes();
    let prefetch = complete.get(..12).ok_or("synthetic prefetch header")?;
    let original = TestBank::new(21)
        .wem(17, complete.as_slice())
        .wem(17, prefetch)
        .wem(18, b"PLUG synthetic media".as_slice())
        .section(*b"UNKN", b"opaque bank data".as_slice())
        .bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(SELECTED, original.as_slice()))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    verify_sample(
        &archive,
        Sample {
            filter: Filter::parse(SELECTED)?,
            entry_index: 0,
            original,
        },
        &schema,
        |original, rows| bank::verify(original, rows).map(|_| ()),
    )
}

fn verify_archives(verify_archive: impl Fn(&Path) -> TestResult) -> TestResult {
    verify_archive(&p4kx_testkit::real_archive_path()?)?;
    if let Some(second) = p4kx_testkit::real_archive_b_path() {
        verify_archive(&second)?;
    }
    Ok(())
}

fn verify_archive(path: &Path) -> TestResult {
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::real_archive_key()?))?;
    let watch = ArchiveWatch::new(&archive);
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for suffix in [".wem", ".bnk"] {
        let sample = sample(&archive, &watch, suffix, |original| {
            if suffix == ".wem" {
                Wem::parse(original).is_ok()
            } else {
                Soundbank::parse(original)
                    .is_ok_and(|bank| !bank.objects().is_empty() && bank.wems().len() > 0)
            }
        })?;
        verify_sample(&archive, sample, &schema, |original, rows| {
            if suffix == ".wem" {
                verify_wem(original, rows)?;
                report_loose_coverage(rows);
            } else {
                bank::verify(original, rows)?;
                report_bank_coverage(rows);
            }
            Ok(())
        })?;
        watch.check()?;
    }
    Ok(())
}

#[expect(
    clippy::print_stderr,
    reason = "the opt-in test reports coverage without source values"
)]
fn report_loose_coverage(rows: &[MediaRecord]) {
    let cue = at(rows, "audio-wem-cue").next().is_some();
    let list = at(rows, "audio-wem-list").next().is_some();
    eprintln!("WEM source coverage: cue={cue}, list={list}");
}

#[expect(
    clippy::print_stderr,
    reason = "the opt-in test reports coverage without source values"
)]
fn report_bank_coverage(rows: &[MediaRecord]) {
    let complete = at(rows, "audio-bank-wem").next().is_some();
    let prefetch = at(rows, "audio-bank-media").any(|row| {
        row.data
            .pointer("/payload/data/prefetch")
            .and_then(Value::as_bool)
            == Some(true)
    });
    let opaque = at(rows, "audio-bank-media").any(|row| {
        matches!(
            row.data
                .pointer("/payload/data/class")
                .and_then(Value::as_str),
            Some("midi" | "impulse" | "unknown")
        )
    });
    eprintln!(
        "DIDX source coverage: complete_wem={complete}, prefetch={prefetch}, opaque={opaque}"
    );
}

fn verify_archive_containers(path: &Path) -> TestResult {
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::real_archive_key()?))?;
    let watch = ArchiveWatch::new(&archive);
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    let sample = sample(
        &archive,
        &watch,
        ".bnk",
        container::has_containers_with_children,
    )?;
    let counts = verify_sample(&archive, sample, &schema, |original, rows| {
        let bank = bank::verify(original, rows)?;
        container::verify(&bank, original, rows)
    })?;
    watch.check()?;
    report_container_counts(counts.containers, counts.children);
    Ok(())
}

#[expect(
    clippy::print_stderr,
    reason = "the opt-in test reports only aggregate coverage"
)]
fn report_container_counts(containers: usize, children: usize) {
    eprintln!("HIRC container records={containers}, child records={children}");
}

fn verify_sample<T>(
    archive: &Archive,
    sample: Sample,
    schema: &jsonschema::Validator,
    compare: impl FnOnce(&[u8], &[MediaRecord]) -> TestResult<T>,
) -> TestResult<T> {
    let mut options = ExportOptions {
        kinds: vec![MediaKind::Audio],
        include: Some(sample.filter),
        exclude: Exclusion::parse(["*.pak", "*.socpak"])?,
        ..ExportOptions::default()
    };
    options.limits.entry_bytes = MAX_SAMPLE_BYTES;
    options.limits.source_bytes = MAX_SAMPLE_BYTES;
    let output = tempfile::tempdir()?;
    let ExportOutcome::Published { index, .. } =
        p4kx_media::export(archive, output.path(), &options)?
    else {
        return Err("one selected audio Entry must publish completely".into());
    };
    let rows = records(
        output.path(),
        &index,
        &sample.original,
        sample.entry_index,
        archive,
        schema,
    )?;
    let result = compare(&sample.original, &rows)?;
    output.close()?;
    Ok(result)
}

struct Sample {
    filter: Filter,
    entry_index: usize,
    original: Vec<u8>,
}

fn sample(
    archive: &Archive,
    watch: &ArchiveWatch,
    suffix: &str,
    applicable: impl Fn(&[u8]) -> bool,
) -> TestResult<Sample> {
    let candidates = archive.entries().iter().enumerate().filter(|(_, entry)| {
        !entry.path().contains(['*', '?'])
            && has_suffix(entry, suffix)
            && (1..=MAX_SAMPLE_BYTES).contains(&entry.uncompressed_size())
    });
    for (entry_index, entry) in candidates.take(MAX_PROBES_PER_KIND) {
        watch.check()?;
        let filter = Filter::parse(entry.path())?;
        let matches = archive.entries().iter().filter(|other| {
            filter.matches(other.path())
                && !has_suffix(other, ".pak")
                && !has_suffix(other, ".socpak")
        });
        if matches.take(2).count() != 1 {
            continue;
        }
        let mut original = Vec::new();
        archive
            .open_entry_verified(entry)?
            .read_to_end(&mut original)?;
        if applicable(&original) {
            return Ok(Sample {
                filter,
                entry_index,
                original,
            });
        }
    }
    Err("no bounded real root audio Entry with the required directory".into())
}

fn has_suffix(entry: &Entry, suffix: &str) -> bool {
    entry
        .path()
        .get(entry.path().len().saturating_sub(suffix.len())..)
        .is_some_and(|actual| actual.eq_ignore_ascii_case(suffix))
}

fn records(
    output: &Path,
    index: &Index,
    original: &[u8],
    ordinal: usize,
    archive: &Archive,
    schema: &jsonschema::Validator,
) -> TestResult<Vec<MediaRecord>> {
    require(index.source_entries.written == 1, "one retained root Entry")?;
    let base = output.join(&index.artifact_base);
    let kind = index
        .kinds
        .iter()
        .find(|kind| kind.provider.kind == MediaKind::Audio)
        .ok_or("audio index kind")?;
    let mut inventory = loose::RootInventory::new(archive, ordinal);
    let mut selected = Vec::new();
    for line in BufReader::new(File::open(base.join(&kind.artifact.path))?).lines() {
        let row: MediaRecord = serde_json::from_str(&line?)?;
        require(
            schema.is_valid(&serde_json::to_value(&row)?),
            "built-in audio schema",
        )?;
        if !inventory.accepts(&row)? {
            selected.push(row);
        }
    }
    inventory.finish()?;
    selected_raw(&base, &selected, original, ordinal)?;
    Ok(selected)
}

fn selected_raw(base: &Path, rows: &[MediaRecord], original: &[u8], ordinal: usize) -> TestResult {
    let raw = rows
        .first()
        .and_then(|row| row.source.raw.as_ref())
        .ok_or("audio raw reference")?;
    require(
        fs::read(base.join(&raw.path))? == original,
        "exact retained audio bytes",
    )?;
    for row in rows {
        let original_identity = matches!(&row.source.identity, Identity::Entry { entry, containers }
            if entry.entry_index == ordinal && containers.is_empty());
        require(
            original_identity && row.source.raw.as_ref() == Some(raw),
            "physical occurrence on every selected audio row",
        )?;
        require(
            row.status == Status::Complete,
            "complete projection on every selected row",
        )?;
    }
    Ok(())
}

fn verify_wem(original: &[u8], rows: &[MediaRecord]) -> TestResult {
    let wem = Wem::parse(original)?;
    let mut summaries = at(rows, "audio-wem");
    let summary = summaries.next().ok_or("standalone WEM summary")?;
    require(summaries.next().is_none(), "one standalone WEM summary")?;
    require(
        summary.data.pointer("/payload/format_tag") == Some(&json!(wem.format_tag())),
        "original WEM format tag",
    )?;
    let actual = at(rows, "audio-wem-chunk").collect::<Vec<_>>();
    let expected = wem.chunks().collect::<Vec<_>>();
    require(
        actual.len() == expected.len(),
        "complete RIFF/RIFX directory",
    )?;
    for (ordinal, (row, chunk)) in actual.into_iter().zip(expected).enumerate() {
        require(
            row.subresource.indices == [u64::try_from(ordinal)?],
            "original WEM chunk position",
        )?;
        require(
            row.data.pointer("/payload/tag") == Some(&json!(chunk.id)),
            "original WEM tag",
        )?;
        require(
            range(row, "/payload/body_range", original)? == chunk.body,
            "original WEM chunk body",
        )?;
        require(
            range(row, "/payload/header_range", original)?.starts_with(&chunk.id),
            "original WEM chunk header",
        )?;
    }
    Ok(())
}

fn at<'rows>(
    rows: &'rows [MediaRecord],
    namespace: &str,
) -> impl Iterator<Item = &'rows MediaRecord> {
    rows.iter()
        .filter(move |row| row.subresource.namespace == namespace)
}

fn range<'input>(row: &MediaRecord, key: &str, input: &'input [u8]) -> TestResult<&'input [u8]> {
    let range = row.data.pointer(key).ok_or("emitted audio source range")?;
    require(
        range.pointer("/basis/type").and_then(Value::as_str) == Some("entry"),
        "Entry-relative audio range",
    )?;
    let start = offset(range, "/start")?;
    let end = offset(range, "/end_exclusive")?;
    input
        .get(start..end)
        .ok_or_else(|| "audio range is outside the retained Entry".into())
}

fn offset(range: &Value, key: &str) -> TestResult<usize> {
    let value = range.pointer(key).ok_or("emitted audio offset")?;
    let number = match value.as_str() {
        Some(decimal) => decimal.parse()?,
        None => value.as_u64().ok_or("unsigned audio offset")?,
    };
    Ok(usize::try_from(number)?)
}
