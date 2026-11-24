// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, BufReader, Seek as _, SeekFrom};

use super::digest_runs::{Fingerprint, OriginDigest, PAIR_BYTES};

const PREFIX_COUNT: usize = 1 << u16::BITS;
const MAX_FILTER_BYTES: usize = 64 * 1024 * 1024;
const BYTES_PER_FILTER_ID: usize = 3;

pub(super) struct DigestIndex {
    file: File,
    count: u64,
    prefix: Vec<u64>,
    filter: MembershipFilter,
}

impl DigestIndex {
    pub(super) fn open(file: File) -> io::Result<Self> {
        let length = file.metadata()?.len();
        if !length.is_multiple_of(PAIR_BYTES) {
            return Err(io::Error::other(
                "the source digest index has a partial record",
            ));
        }
        let count = length / PAIR_BYTES;
        let mut index = Self {
            file,
            count,
            prefix: vec![0; PREFIX_COUNT + 1],
            filter: MembershipFilter::new(count),
        };
        index.build()?;
        Ok(index)
    }

    fn build(&mut self) -> io::Result<()> {
        let mut reader = self.reader()?;
        let mut previous = None;
        let mut prefix_start = 0;
        for ordinal in 0..self.count {
            let pair = OriginDigest::read(&mut reader)?
                .ok_or_else(|| io::Error::other("missing source digest index row"))?;
            if previous.is_some_and(|id| id >= pair.id) {
                return Err(io::Error::other("source digest index IDs are out of order"));
            }
            let prefix_end = prefix(&pair.id) + 1;
            self.prefix
                .get_mut(prefix_start..prefix_end)
                .ok_or_else(|| io::Error::other("invalid source digest prefix range"))?
                .fill(ordinal);
            prefix_start = prefix_end;
            self.filter.insert(&pair.id)?;
            previous = Some(pair.id);
        }
        self.prefix
            .get_mut(prefix_start..)
            .ok_or_else(|| io::Error::other("invalid terminal source digest prefix"))?
            .fill(self.count);
        Ok(())
    }

    pub(super) const fn count(&self) -> u64 {
        self.count
    }

    pub(super) fn reader(&self) -> io::Result<BufReader<File>> {
        let mut file = self.file.try_clone()?;
        file.rewind()?;
        Ok(BufReader::new(file))
    }

    pub(super) fn lookup(
        &self,
        reader: &mut BufReader<File>,
        id: &Fingerprint,
    ) -> io::Result<Option<Fingerprint>> {
        if !self.filter.possibly_contains(id) {
            return Ok(None);
        }
        let prefix = prefix(id);
        let Some(&[mut start, mut end]) = self.prefix.get(prefix..=prefix + 1) else {
            return Err(io::Error::other("missing source digest range"));
        };
        while start < end {
            let middle = start + (end - start) / 2;
            reader.seek(SeekFrom::Start(middle * PAIR_BYTES))?;
            let pair = OriginDigest::read(reader)?
                .ok_or_else(|| io::Error::other("missing indexed source digest"))?;
            match pair.id.cmp(id) {
                std::cmp::Ordering::Less => start = middle + 1,
                std::cmp::Ordering::Greater => end = middle,
                std::cmp::Ordering::Equal => return Ok(Some(pair.payload)),
            }
        }
        Ok(None)
    }
}

fn prefix(&[first, second, ..]: &Fingerprint) -> usize {
    usize::from(u16::from_be_bytes([first, second]))
}

struct MembershipFilter {
    bytes: Vec<u8>,
}

impl MembershipFilter {
    fn new(count: u64) -> Self {
        let size = usize::try_from(count)
            .unwrap_or(usize::MAX)
            .saturating_mul(BYTES_PER_FILTER_ID)
            .clamp(1, MAX_FILTER_BYTES)
            .next_power_of_two();
        Self {
            bytes: vec![0; size],
        }
    }

    fn insert(&mut self, id: &Fingerprint) -> io::Result<()> {
        for (index, mask) in filter_slots(id, self.bytes.len()) {
            let byte = self
                .bytes
                .get_mut(index)
                .ok_or_else(|| io::Error::other("invalid source ID filter index"))?;
            *byte |= mask;
        }
        Ok(())
    }

    fn possibly_contains(&self, id: &Fingerprint) -> bool {
        filter_slots(id, self.bytes.len())
            .all(|(index, mask)| self.bytes.get(index).is_some_and(|byte| byte & mask != 0))
    }
}

fn filter_slots(id: &Fingerprint, length: usize) -> impl Iterator<Item = (usize, u8)> {
    const PROBES: usize = 5;
    let (words, _) = id.as_chunks::<4>();
    words.iter().take(PROBES).map(move |&[a, b, c, d]| {
        let upper = usize::from(u16::from_be_bytes([a, b]));
        let lower = usize::from(u16::from_be_bytes([c, d]));
        let bit = ((upper << 16) | lower) & (length * 8 - 1);
        (bit / 8, 1 << (bit % 8))
    })
}

#[test]
fn lookups_use_exact_digest_identity_across_empty_and_shared_prefix_ranges() -> io::Result<()> {
    use super::digest_runs::DigestRuns;

    let scratch = tempfile::tempdir()?;
    let first = [0_u8; 32];
    let mut second = [0_u8; 32];
    let mut missing = [0_u8; 32];
    if let Some(last) = second.last_mut() {
        *last = 1;
    }
    if let Some(last) = missing.last_mut() {
        *last = 2;
    }
    let last = [u8::MAX; 32];
    let mut runs = DigestRuns::new(scratch.path());
    for id in [last, second, first] {
        runs.push(OriginDigest { id, payload: id })?;
    }
    let index = DigestIndex::open(runs.finish()?)?;
    let mut reader = index.reader()?;
    for id in [second, first, last, second] {
        assert_eq!(
            index.lookup(&mut reader, &id)?,
            Some(id),
            "exact membership in nonsorted lookup order"
        );
    }
    for id in [missing, [1; 32], [128; 32]] {
        assert_eq!(
            index.lookup(&mut reader, &id)?,
            None,
            "nonmembers are refused even on a filter collision"
        );
    }
    let empty = DigestIndex::open(DigestRuns::new(scratch.path()).finish()?)?;
    assert_eq!(
        empty.lookup(&mut empty.reader()?, &first)?,
        None,
        "empty range"
    );
    Ok(())
}
