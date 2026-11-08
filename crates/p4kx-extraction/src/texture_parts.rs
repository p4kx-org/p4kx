// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use p4kx_archive::Archive;
use p4kx_texture::SiblingReader;

use crate::disk_parts::DiskParts;
use crate::entry_path::same_entry_path;
use crate::path_index::PathIndex;
use crate::source::{EntrySource, SourceEntry, SourceReader};

const ALPHA_SUFFIX: &str = "a";
const MAX_PROBED_PARTS: u32 = 32;

/// Where the header and the parts of one split texture are stored.
#[derive(Debug, Clone)]
pub(crate) enum Parts<'a> {
    /// Files of a source, by part number.
    Source {
        header: SourceEntry<'a>,
        parts: BTreeMap<u32, SourceEntry<'a>>,
        verify: bool,
    },
    /// Disk files with shared case resolution and retained physical paths.
    Files(DiskParts),
}

/// The parts of a texture and of its alpha tail, if it has one.
#[derive(Debug)]
pub(crate) struct Found<'a> {
    pub(crate) colour: Parts<'a>,
    pub(crate) alpha: Option<Parts<'a>>,
}

/// The data of one part.
pub(crate) enum PartReader<'a> {
    Source(SourceReader<'a>),
    File(File),
}

impl Read for PartReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Source(reader) => reader.read(buffer),
            Self::File(file) => file.read(buffer),
        }
    }
}

impl<'a> Parts<'a> {
    /// Finds the Entry at `path`, its parts and its alpha tail in one parallel pass over the
    /// Archive, or `None` when no Entry has that path.
    ///
    /// A path ending in `.a` names an alpha tail itself; its parts are `.1a` . `.Na` and it has
    /// no alpha tail of its own.
    pub(crate) fn in_archive(archive: &'a Archive, path: &str) -> Option<Found<'a>> {
        let (base, is_alpha) = split_alpha(path);
        let source: &'a dyn EntrySource = archive;
        let found: Vec<(Role, SourceEntry<'a>)> = source
            .members(&|candidate| classify(candidate, base).is_some())
            .into_iter()
            .filter_map(|entry| classify(entry.path(), base).map(|role| (role, entry)))
            .collect();
        let find = |wanted: Role| {
            found
                .iter()
                .find_map(|(role, entry)| (*role == wanted).then_some(*entry))
        };
        let parts_of = |alpha: bool| {
            let mut parts = BTreeMap::new();
            for (role, entry) in &found {
                if let Role::Part { index, alpha: is } = *role
                    && is == alpha
                {
                    parts.entry(index).or_insert(*entry);
                }
            }
            parts
        };
        let header = find(if is_alpha {
            Role::AlphaHeader
        } else {
            Role::Header
        })?;
        let alpha = if is_alpha {
            None
        } else {
            find(Role::AlphaHeader).map(|header| Self::Source {
                header,
                parts: parts_of(true),
                verify: false,
            })
        };
        Some(Found {
            colour: Self::Source {
                header,
                parts: parts_of(is_alpha),
                verify: false,
            },
            alpha,
        })
    }

    /// Finds the Entry at `path`, its parts and its alpha tail through `index`, probing the
    /// parts `.1`, `.2`, . until one is missing instead of scanning the Archive; `path` is
    /// treated as [`Parts::in_archive`] treats it.
    pub(crate) fn in_index(index: &PathIndex<'a>, path: &str) -> Option<Found<'a>> {
        let (base, is_alpha) = split_alpha(path);
        let archive = index.archive();
        let header = index
            .find(&if is_alpha {
                format!("{base}.{ALPHA_SUFFIX}")
            } else {
                base.to_owned()
            })
            .map(|entry| SourceEntry::of_archive(archive, entry))?;
        let parts_of = |suffix: &str| {
            let mut parts = BTreeMap::new();
            for number in 1..=MAX_PROBED_PARTS {
                let Some(part) = index.find(&format!("{base}.{number}{suffix}")) else {
                    break;
                };
                parts.insert(number, SourceEntry::of_archive(archive, part));
            }
            parts
        };
        let colour_suffix = if is_alpha { ALPHA_SUFFIX } else { "" };
        let alpha = if is_alpha {
            None
        } else {
            index
                .find(&format!("{base}.{ALPHA_SUFFIX}"))
                .map(|alpha| Self::Source {
                    header: SourceEntry::of_archive(archive, alpha),
                    parts: parts_of(ALPHA_SUFFIX),
                    verify: false,
                })
        };
        Some(Found {
            colour: Self::Source {
                header,
                parts: parts_of(colour_suffix),
                verify: false,
            },
            alpha,
        })
    }
}

impl SiblingReader for Parts<'_> {
    type Part<'r>
        = PartReader<'r>
    where
        Self: 'r;

    fn part_len(&self, index: u32) -> io::Result<Option<u64>> {
        match self {
            Self::Source { parts, .. } => Ok(parts.get(&index).map(SourceEntry::uncompressed_size)),
            Self::Files(parts) => parts.part_len(index),
        }
    }

    fn open_part(&self, index: u32) -> io::Result<Self::Part<'_>> {
        match self {
            Self::Source { parts, verify, .. } => {
                let entry = parts
                    .get(&index)
                    .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
                entry
                    .open(*verify)
                    .map(PartReader::Source)
                    .map_err(io::Error::other)
            }
            Self::Files(parts) => parts.open_part(index).map(PartReader::File),
        }
    }
}

/// What an Entry is to the texture being looked up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Header,
    AlphaHeader,
    Part { index: u32, alpha: bool },
}

/// Returns `path` without a trailing `.a`, and whether it had one.
fn split_alpha(path: &str) -> (&str, bool) {
    match path
        .len()
        .checked_sub(2)
        .and_then(|at| path.split_at_checked(at))
    {
        Some((base, ".a" | ".A")) => (base, true),
        _ => (path, false),
    }
}

/// Returns the role of `candidate` for the texture whose header path is `base`, or `None` when it
/// is neither the header, its alpha tail nor one of their parts; separators and ASCII case do
/// not matter.
fn classify(candidate: &str, base: &str) -> Option<Role> {
    let (start, rest) = candidate.split_at_checked(base.len())?;
    if !same_entry_path(start, base) {
        return None;
    }
    let Some(suffix) = rest.strip_prefix('.') else {
        return rest.is_empty().then_some(Role::Header);
    };
    if suffix.eq_ignore_ascii_case(ALPHA_SUFFIX) {
        return Some(Role::AlphaHeader);
    }
    let (digits, alpha) = suffix
        .strip_suffix(['a', 'A'])
        .map_or((suffix, false), |digits| (digits, true));
    if digits.starts_with('0') || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok().map(|index| Role::Part { index, alpha })
}

/// Returns whether `path` names an existing file that is one of `inputs`.
pub(crate) fn is_same_file(path: &Path, inputs: impl IntoIterator<Item = PathBuf>) -> bool {
    let Ok(target) = fs::canonicalize(path) else {
        return false;
    };
    inputs
        .into_iter()
        .filter_map(|input| fs::canonicalize(input).ok())
        .any(|input| input == target)
}

#[cfg(test)]
mod tests {
    use super::{Role, classify, split_alpha};

    #[test]
    fn parts_and_the_alpha_tail_are_siblings_of_the_header() {
        let header = "Data\\a.dds";
        let part = |index, alpha| Some(Role::Part { index, alpha });
        for (candidate, role) in [
            ("data/A.DDS", Some(Role::Header)),
            ("Data\\a.dds.1", part(1, false)),
            ("Data\\a.dds.12", part(12, false)),
            ("Data\\a.dds.a", Some(Role::AlphaHeader)),
            ("Data/A.DDS.A", Some(Role::AlphaHeader)),
            ("Data\\a.dds.1a", part(1, true)),
            ("Data\\a.dds.7A", part(7, true)),
        ] {
            assert_eq!(classify(candidate, header), role, "{candidate}");
        }
        for other in [
            "Data\\a.dds.0",
            "Data\\a.dds.01",
            "Data\\a.dds.0a",
            "Data\\a.dds.aa",
            "Data\\a.dds.1b",
            "Data\\a.dds.",
            "Data\\a.ddsx",
            "Data\\b.dds.1",
            "Data\\a.dds.99999999999",
        ] {
            assert_eq!(classify(other, header), None, "{other} is no part");
        }
    }

    #[test]
    fn a_path_ending_in_a_names_an_alpha_tail() {
        assert_eq!(split_alpha("x.dds.a"), ("x.dds", true), "lower case");
        assert_eq!(split_alpha("x.dds.A"), ("x.dds", true), "upper case");
        assert_eq!(split_alpha("x.dds"), ("x.dds", false), "a colour header");
        assert_eq!(split_alpha("a"), ("a", false), "too short");
    }
}
