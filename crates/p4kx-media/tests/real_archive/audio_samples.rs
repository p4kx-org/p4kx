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
    candidates(archive, suffix)
        .take(MAX_PROBES_PER_KIND)
        .filter_map(move |(index, entry)| read_sample(archive, watch, index, entry).transpose())
}

pub(super) fn survey<'archive>(
    archive: &'archive Archive,
    watch: &'archive ArchiveWatch,
    suffix: &'archive str,
) -> impl Iterator<Item = TestResult<(usize, Vec<u8>)>> + 'archive {
    candidates(archive, suffix).map(move |(index, entry)| {
        read_original(archive, watch, entry).map(|original| (index, original))
    })
}

pub(super) fn survey_sample(
    archive: &Archive,
    index: usize,
    original: Vec<u8>,
) -> TestResult<Option<Sample>> {
    let entry = archive
        .entries()
        .get(index)
        .ok_or("physical root audio Entry")?;
    Ok(unique_filter(archive, entry)?.map(|filter| Sample {
        filter,
        entry_index: index,
        original,
    }))
}

fn candidates<'archive>(
    archive: &'archive Archive,
    suffix: &'archive str,
) -> impl Iterator<Item = (usize, &'archive Entry)> + 'archive {
    archive
        .entries()
        .iter()
        .enumerate()
        .filter(move |(_, entry)| {
            !entry.path().contains(['*', '?'])
                && has_suffix(entry, suffix)
                && (1..=MAX_SAMPLE_BYTES).contains(&entry.uncompressed_size())
        })
}

fn read_sample(
    archive: &Archive,
    watch: &ArchiveWatch,
    entry_index: usize,
    entry: &Entry,
) -> TestResult<Option<Sample>> {
    let Some(filter) = unique_filter(archive, entry)? else {
        return Ok(None);
    };
    let original = read_original(archive, watch, entry)?;
    Ok(Some(Sample {
        filter,
        entry_index,
        original,
    }))
}

fn unique_filter(archive: &Archive, entry: &Entry) -> TestResult<Option<Filter>> {
    let filter = Filter::parse(entry.path())?;
    let matches = archive.entries().iter().filter(|other| {
        filter.matches(other.path()) && !has_suffix(other, ".pak") && !has_suffix(other, ".socpak")
    });
    Ok((matches.take(2).count() == 1).then_some(filter))
}

fn read_original(archive: &Archive, watch: &ArchiveWatch, entry: &Entry) -> TestResult<Vec<u8>> {
    watch.poll()?;
    let mut original = Vec::new();
    archive
        .open_entry_verified(entry)?
        .read_to_end(&mut original)?;
    Ok(original)
}

fn has_suffix(entry: &Entry, suffix: &str) -> bool {
    entry
        .path()
        .get(entry.path().len().saturating_sub(suffix.len())..)
        .is_some_and(|actual| actual.eq_ignore_ascii_case(suffix))
}
