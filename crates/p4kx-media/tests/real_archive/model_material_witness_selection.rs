// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;
use std::io::Read;
use std::ops::Range;

use p4kx_extraction::chunks::{ChunkFile, crch, ivo};
use p4kx_extraction::{Archive, ArchiveWatch, Entry, PathIndex};

use super::ENTRY_BYTES;
use super::paths::{self, MaterialPath};
use crate::TestResult;

#[path = "model_material_witness_cdf.rs"]
mod cdf;

const PER_FAMILY_PROBES: usize = 96;
const MATERIAL_NAME_BYTES: usize = 128;
const CRCH_NAME_VERSION: u32 = 0x802;

#[derive(Clone, Copy)]
pub(super) enum Family {
    Packed,
    Cdf,
}

impl Family {
    pub(super) const fn namespace(self) -> &'static str {
        match self {
            Self::Packed => "model-material-library",
            Self::Cdf => "model-character-attachment",
        }
    }

    pub(super) const fn reference(self) -> &'static str {
        match self {
            Self::Packed => "/payload/reference",
            Self::Cdf => "/payload/material_reference",
        }
    }

    fn admits(self, name: &str) -> bool {
        let extension = name.rsplit_once('.').map(|(_, ext)| ext);
        match self {
            Self::Packed => extension.is_some_and(model_extension),
            Self::Cdf => extension.is_some_and(|ext| ext.eq_ignore_ascii_case("cdf")),
        }
    }
}

pub(super) struct Sample<'a> {
    pub(super) entry: &'a Entry,
    pub(super) bytes: Vec<u8>,
    pub(super) material: &'a Entry,
    pub(super) binding: Option<Binding<'a>>,
    pub(super) binding_used: bool,
    pub(super) authored: String,
    pub(super) indices: Vec<u64>,
    pub(super) range: Option<Range<u64>>,
    pub(super) location: Location,
}

impl<'a> Sample<'a> {
    pub(super) fn targets(&self) -> impl Iterator<Item = &'a Entry> + Clone {
        std::iter::once(self.material).chain(self.binding.as_ref().map(|binding| binding.target))
    }
}

pub(super) struct Binding<'a> {
    pub(super) target: &'a Entry,
    pub(super) authored: String,
}

pub(super) struct Location {
    pub(super) namespace: &'static str,
    pub(super) field: String,
    pub(super) indices: [u64; 2],
}

pub(super) struct Original<'a> {
    archive: &'a Archive,
    watch: &'a ArchiveWatch,
    index: PathIndex<'a>,
}

impl<'a> Original<'a> {
    pub(super) fn new(archive: &'a Archive, watch: &'a ArchiveWatch) -> Self {
        Self {
            archive,
            watch,
            index: PathIndex::new(archive),
        }
    }

    pub(super) fn select(
        &self,
        family: Family,
        counts: &mut ProbeCounts,
    ) -> TestResult<Option<Sample<'a>>> {
        self.select_matching(family, counts, |_| true)
    }

    pub(super) fn select_matching(
        &self,
        family: Family,
        counts: &mut ProbeCounts,
        include: impl Fn(&Entry) -> bool,
    ) -> TestResult<Option<Sample<'a>>> {
        let eligible = |entry: &&Entry| {
            entry.uncompressed_size() <= ENTRY_BYTES
                && (!entry.is_encrypted() || self.archive.has_key())
                && self.archive.raw_path(entry.index()).is_none()
                && !entry.path().contains(['*', '?'])
                && family.admits(entry.path())
                && include(entry)
        };
        counts.eligible = self.archive.entries().iter().filter(eligible).count();
        let total = counts.eligible.min(PER_FAMILY_PROBES);
        let mut entries = self.archive.entries().iter().filter(eligible);
        let mut cursor = 0;
        for index in 0..total {
            self.watch.poll()?;
            let offset = index * counts.eligible / total;
            let entry = entries
                .nth(offset - cursor)
                .ok_or("bounded candidate ordinal")?;
            cursor = offset + 1;
            counts.probed += 1;
            let Some(bytes) = self.verified(entry, counts) else {
                continue;
            };
            let observation = match family {
                Family::Packed => self.packed(entry, &bytes, counts)?,
                Family::Cdf => self.cdf(&bytes, counts)?,
            };
            if let Some(found) = observation {
                return Ok(Some(Sample {
                    entry,
                    bytes,
                    material: found.material,
                    binding: found.binding,
                    binding_used: found.binding_used,
                    authored: found.authored,
                    indices: found.indices,
                    range: found.range,
                    location: found.location,
                }));
            }
        }
        Ok(None)
    }

    fn verified(&self, entry: &Entry, counts: &mut ProbeCounts) -> Option<Vec<u8>> {
        let Ok(mut reader) = self.archive.open_entry_verified(entry) else {
            counts.open_failed += 1;
            return None;
        };
        let mut bytes = Vec::new();
        if reader.read_to_end(&mut bytes).is_err() {
            counts.decode_failed += 1;
            return None;
        }
        counts.verified += 1;
        Some(bytes)
    }

    fn packed(
        &self,
        entry: &Entry,
        bytes: &[u8],
        counts: &mut ProbeCounts,
    ) -> TestResult<Option<Observation<'a>>> {
        let Ok(file) = ChunkFile::parse(bytes) else {
            return Ok(None);
        };
        counts.recognized += 1;
        let ordinal = [ivo::MTL_NAME_320, ivo::MTL_NAME, crch::MTL_NAME]
            .into_iter()
            .find_map(|expected| {
                file.chunks().iter().position(|chunk| {
                    chunk.chunk_type() == expected
                        && (expected != crch::MTL_NAME || chunk.version() == CRCH_NAME_VERSION)
                })
            });
        let Some(ordinal) = ordinal else {
            return Ok(None);
        };
        let raw = file.chunk_data(ordinal).ok_or("original name chunk")?;
        let prefix = raw
            .get(..MATERIAL_NAME_BYTES.min(raw.len()))
            .ok_or("bounded name bytes")?;
        let token = prefix
            .split(|byte| *byte == 0)
            .next()
            .ok_or("original name field")?;
        let Ok(authored) = std::str::from_utf8(token) else {
            return Ok(None);
        };
        counts.declarations += 1;
        let Some(path) = material_path(authored.trim(), false, counts) else {
            return Ok(None);
        };
        let Some(material) = self.material(Some(entry), &path)? else {
            return Ok(None);
        };
        let chunk = file
            .chunks()
            .get(ordinal)
            .ok_or("original physical chunk")?;
        let start = chunk.offset();
        let end = start + token.len() as u64;
        Ok(Some(Observation {
            material,
            binding: None,
            binding_used: false,
            authored: authored.to_owned(),
            indices: vec![ordinal as u64],
            range: Some(start..end),
            location: Location {
                namespace: "model-byte-range",
                field: "modelMaterialName".into(),
                indices: [start, end],
            },
        }))
    }

    fn material(
        &self,
        model: Option<&Entry>,
        path: &MaterialPath,
    ) -> TestResult<Option<&'a Entry>> {
        if path.beside_model {
            let Some(model) = model else {
                return Ok(None);
            };
            let model_dir = model
                .path()
                .rsplit_once(['/', '\\'])
                .map_or("", |(folder, _)| folder);
            let sibling = if model_dir.is_empty() {
                path.filename.clone()
            } else {
                format!("{model_dir}/{}", path.filename)
            };
            match self.path_match(&sibling)? {
                Match::Missing => {}
                found => return Ok(found.unique()),
            }
        }
        Ok(self
            .path_match(&format!("Data/{}", path.filename))?
            .unique())
    }

    fn path_match(&self, path: &str) -> TestResult<Match<'a>> {
        let mut matches = self
            .index
            .matching_entries_with(path, |_| Ok::<(), Infallible>(()))?;
        let Some(first) = matches.next().transpose()? else {
            return Ok(Match::Missing);
        };
        Ok(
            if self.archive.raw_path(first.index()).is_some()
                || matches.next().transpose()?.is_some()
            {
                Match::Unavailable
            } else {
                Match::Unique(first)
            },
        )
    }
}

struct Observation<'a> {
    material: &'a Entry,
    binding: Option<Binding<'a>>,
    binding_used: bool,
    authored: String,
    indices: Vec<u64>,
    range: Option<Range<u64>>,
    location: Location,
}

enum Match<'a> {
    Missing,
    Unavailable,
    Unique(&'a Entry),
}

impl<'a> Match<'a> {
    const fn unique(self) -> Option<&'a Entry> {
        match self {
            Self::Unique(entry) => Some(entry),
            Self::Missing | Self::Unavailable => None,
        }
    }
}

fn material_path(
    authored: &str,
    fold_leading_backslash: bool,
    counts: &mut ProbeCounts,
) -> Option<MaterialPath> {
    let path = paths::authored(authored, fold_leading_backslash)?;
    counts.valid_paths += 1;
    Some(path)
}

#[derive(Debug, Default)]
pub(super) struct ProbeCounts {
    eligible: usize,
    probed: usize,
    open_failed: usize,
    decode_failed: usize,
    verified: usize,
    recognized: usize,
    declarations: usize,
    valid_paths: usize,
    binding_unavailable: usize,
}

fn model_extension(value: &str) -> bool {
    ["cgf", "cga", "chr", "skin"]
        .iter()
        .any(|ext| value.eq_ignore_ascii_case(ext))
}
