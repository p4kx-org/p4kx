// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Read, Write as _};
use std::path::Path;

use p4kx_extraction::{Archive, ArchiveWatch, Entry, PathIndex, TextureMemberKeys};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::{TestResult, privacy};

#[path = "texture_accounting_compare.rs"]
mod compare;
#[path = "texture_accounting_survey.rs"]
mod survey;

const MAX_REPLAY_BYTES: u64 = 64 << 20;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn texture_headers_and_stratified_media_outputs_match_original_reads()
-> Result<(), privacy::VerificationFailure> {
    privacy::redact(verify())
}

fn verify() -> TestResult {
    let archive = Archive::open_with_key(
        p4kx_testkit::real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let watch = ArchiveWatch::new(&archive);
    watch.check()?;
    let paths = PathIndex::new(&archive);
    let survey = survey::read(&archive, &paths, &watch)?;
    for transfer in [survey::Class::LinearPng, survey::Class::SrgbPng] {
        if !survey.samples.contains_key(&transfer) {
            return Err("installed root lacks a representative tagged PNG transfer".into());
        }
    }
    let selected: BTreeMap<_, _> = survey
        .samples
        .values()
        .map(|entry| (entry.index(), *entry))
        .collect();
    if selected.is_empty() {
        return Err("installed root holds no bounded convertible DDS strata".into());
    }
    let output = tempfile::tempdir()?;
    let replay = family_replay(&archive, selected.values().copied(), output.path())?;
    let replay_paths = PathIndex::new(&replay);
    for entry in selected.values() {
        let copied = replay_paths
            .find(entry.path())
            .ok_or("verified family replay Entry")?;
        compare::one(&replay, &paths, copied, entry, output.path())?;
        watch.check()?;
    }
    if std::env::var_os("P4KX_TEXTURE_ORIGINAL_ROOT").is_some() {
        let entry = selected.values().next().ok_or("original root witness")?;
        let direct = output.path().join("original-root");
        compare::one(&archive, &paths, entry, entry, &direct)?;
        watch.check()?;
    }
    writeln!(
        io::stdout().lock(),
        "Verified Media conversion in {} source strata with {} distinct DDS families",
        survey.samples.len(),
        selected.len()
    )?;
    drop(replay_paths);
    drop(replay);
    output.close()?;
    Ok(())
}

fn family_replay<'a>(
    archive: &Archive,
    selected: impl Iterator<Item = &'a Entry>,
    output: &Path,
) -> TestResult<Archive> {
    let keys = TextureMemberKeys::try_new_with(archive, |_| Ok::<_, io::Error>(()))?;
    let members = keys.index(archive);
    let mut seen = BTreeSet::new();
    let mut total_bytes = 0_u64;
    let mut builder = ArchiveBuilder::new();
    for entry in selected {
        for candidate in members.family_candidates_with(entry.path(), |_| Ok::<_, io::Error>(()))? {
            let member = candidate?.entry;
            if seen.insert(member.index()) {
                total_bytes = total_bytes
                    .checked_add(member.uncompressed_size())
                    .ok_or("sample size overflow")?;
                if total_bytes > MAX_REPLAY_BYTES {
                    return Err("private installed texture comparison exceeds 64 MiB".into());
                }
                builder = builder.entry(verified_entry(archive, member)?);
            }
        }
    }
    let archive_path = output.join("Data.p4k");
    builder.write_to(&archive_path)?;
    Ok(Archive::open_with_key(
        archive_path,
        Some(p4kx_testkit::test_key()),
    )?)
}

fn verified_entry(archive: &Archive, entry: &Entry) -> TestResult<TestEntry> {
    let mut data = Vec::new();
    archive
        .open_entry_verified(entry)?
        .reject_overlong()
        .read_to_end(&mut data)?;
    let mut copy = TestEntry::stored(entry.path(), data);
    if entry.is_encrypted() {
        copy = copy.encrypted();
    }
    if let Some(raw) = archive.raw_path(entry.index()) {
        copy = copy.with_path_bytes(raw);
    }
    Ok(copy)
}
