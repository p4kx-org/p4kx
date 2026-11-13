// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read;

use p4kx_extraction::{Archive, ArchiveWatch, Entry, Filter};

use super::{MAX_SAMPLE_BYTES, Sample, TestResult};

const MAX_PROBES_PER_KIND: usize = 32;

pub(super) fn sample(
    archive: &Archive,
    watch: &ArchiveWatch,
    suffix: &str,
    applicable: impl Fn(&[u8]) -> bool,
) -> TestResult<Sample> {
    for sample in samples(archive, watch, suffix) {
        let sample = sample?;
        if applicable(&sample.original) {
            return Ok(sample);
        }
    }
    Err("no bounded real root audio Entry with the required directory".into())
}

pub(super) fn samples<'archive>(
    archive: &'archive Archive,
    watch: &'archive ArchiveWatch,
    suffix: &'archive str,
) -> impl Iterator<Item = TestResult<Sample>> + 'archive {
    archive
        .entries()
        .iter()
        .enumerate()
        .filter(move |(_, entry)| {
            !entry.path().contains(['*', '?'])
                && has_suffix(entry, suffix)
                && (1..=MAX_SAMPLE_BYTES).contains(&entry.uncompressed_size())
        })
        .take(MAX_PROBES_PER_KIND)
        .filter_map(move |(entry_index, entry)| {
            read_sample(archive, watch, entry_index, entry).transpose()
        })
}

fn read_sample(
    archive: &Archive,
    watch: &ArchiveWatch,
    entry_index: usize,
    entry: &Entry,
) -> TestResult<Option<Sample>> {
    watch.check()?;
    let filter = Filter::parse(entry.path())?;
    let matches = archive.entries().iter().filter(|other| {
        filter.matches(other.path()) && !has_suffix(other, ".pak") && !has_suffix(other, ".socpak")
    });
    if matches.take(2).count() != 1 {
        return Ok(None);
    }
    let mut original = Vec::new();
    archive
        .open_entry_verified(entry)?
        .read_to_end(&mut original)?;
    Ok(Some(Sample {
        filter,
        entry_index,
        original,
    }))
}

fn has_suffix(entry: &Entry, suffix: &str) -> bool {
    entry
        .path()
        .get(entry.path().len().saturating_sub(suffix.len())..)
        .is_some_and(|actual| actual.eq_ignore_ascii_case(suffix))
}
