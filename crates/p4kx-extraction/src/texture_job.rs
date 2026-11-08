// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::sound_job::sound_role;
use crate::source::{EntrySource, SourceEntry};
use crate::texture_images::IMAGE_EXTENSIONS;

/// An Entry of the run with its position in the requested order.
pub(crate) type Indexed<'a> = (usize, &'a SourceEntry<'a>);

/// One unit of work of an Extraction.
pub(crate) enum Item<'a> {
    /// An Entry written on its own.
    Entry(Indexed<'a>),
    /// A DDS texture converted as a whole.
    Texture(TextureJob<'a>),
    /// A WEM sound converted as the run's audio option says.
    Sound(Indexed<'a>),
    /// A soundbank written unchanged and unpacked into a folder of its WEMs.
    Bank(Indexed<'a>),
}

/// A texture of the run: its header Entry, the Entries of the texture that were requested with
/// it, and every part and alpha tail Entry the Archive holds for it.
pub(crate) struct TextureJob<'a> {
    pub(crate) header: Indexed<'a>,
    pub(crate) requested: Vec<Indexed<'a>>,
    pub(crate) parts: BTreeMap<u32, SourceEntry<'a>>,
    pub(crate) alpha_header: Option<SourceEntry<'a>>,
    pub(crate) alpha_parts: BTreeMap<u32, SourceEntry<'a>>,
}

impl<'a> TextureJob<'a> {
    const fn new(header: Indexed<'a>) -> Self {
        Self {
            header,
            requested: Vec::new(),
            parts: BTreeMap::new(),
            alpha_header: None,
            alpha_parts: BTreeMap::new(),
        }
    }

    /// Returns every Entry of the run that belongs to the texture, the header first.
    pub(crate) fn entries(&self) -> impl Iterator<Item = Indexed<'a>> + '_ {
        std::iter::once(self.header).chain(self.requested.iter().copied())
    }

    fn add(&mut self, member: Member, entry: SourceEntry<'a>) {
        match member {
            Member::Part(number) => {
                self.parts.entry(number).or_insert(entry);
            }
            Member::AlphaHeader => {
                self.alpha_header.get_or_insert(entry);
            }
            Member::AlphaPart(number) => {
                self.alpha_parts.entry(number).or_insert(entry);
            }
        }
    }
}

impl Item<'_> {
    /// Returns the position of the item's first Entry in the requested order.
    pub(crate) const fn first_index(&self) -> usize {
        match self {
            Self::Entry((index, _)) | Self::Sound((index, _)) | Self::Bank((index, _)) => *index,
            Self::Texture(job) => job.header.0,
        }
    }

    /// Calls `visit` with the Entry path of each file the item may write below the target, and
    /// with a trailing separator for a folder it fills with files not known in advance, and with
    /// a trailing `*` for the names that start with a prefix, as the images of a texture do.
    pub(crate) fn output_paths(&self, mut visit: impl FnMut(&str)) {
        match self {
            Self::Entry((_, entry)) => visit(entry.path()),
            Self::Sound((_, entry)) => {
                visit(entry.path());
                if let Some((_, stem)) = sound_role(entry.path()) {
                    visit(&format!("{stem}.ogg"));
                    visit(&format!("{stem}.wav"));
                }
            }
            Self::Bank((_, entry)) => {
                visit(entry.path());
                if let Some((_, stem)) = sound_role(entry.path()) {
                    visit(&format!("{stem}\\"));
                }
            }
            Self::Texture(job) => {
                let header = job.header.1.path();
                let stem = header
                    .get(..header.len().saturating_sub(4))
                    .unwrap_or_default();
                for extension in IMAGE_EXTENSIONS {
                    visit(&format!("{stem}.{extension}"));
                }
                visit(&format!("{stem}_*"));
                visit(&format!("{header}.a"));
                visit(header);
                for (_, entry) in &job.requested {
                    visit(entry.path());
                }
            }
        }
    }
}

/// Splits `entries` into items: each requested `.dds` header with its requested parts and alpha
/// tail Entries and those the Archive holds becomes one texture, every other Entry stays on its
/// own.
pub(crate) fn group_textures<'a>(
    source: &'a dyn EntrySource,
    entries: &'a [SourceEntry<'a>],
) -> Vec<Item<'a>> {
    let mut headers = Headers::default();
    for (index, entry) in entries.iter().enumerate() {
        if role(entry.path()) == Some(Role::Header) {
            headers.insert(entry.path(), index);
        }
    }
    if headers.by_key.is_empty() {
        return entries.iter().enumerate().map(Item::Entry).collect();
    }
    let mut jobs: HashMap<usize, TextureJob<'a>> = headers
        .by_key
        .values()
        .filter_map(|&index| Some((index, TextureJob::new((index, entries.get(index)?)))))
        .collect();
    for (header, member, entry) in source_members(source, &headers) {
        if let Some(job) = jobs.get_mut(&header) {
            job.add(member, entry);
        }
    }
    let mut items = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        if jobs.contains_key(&index) {
            continue;
        }
        let owner = match role(entry.path()) {
            Some(Role::Member(header_path, _)) => headers.find(header_path),
            _ => None,
        };
        match owner.and_then(|header| jobs.get_mut(&header)) {
            Some(job) => job.requested.push((index, entry)),
            None => items.push(Item::Entry((index, entry))),
        }
    }
    let mut jobs: Vec<_> = jobs.into_values().collect();
    jobs.sort_unstable_by_key(|job| job.header.0);
    items.extend(jobs.into_iter().map(Item::Texture));
    items.sort_unstable_by_key(Item::first_index);
    items
}

/// The requested headers by their key, with the lengths of their paths to skip most other paths
/// without building a key.
#[derive(Default)]
struct Headers {
    by_key: HashMap<String, usize>,
    lengths: HashSet<usize>,
}

impl Headers {
    fn insert(&mut self, path: &str, index: usize) {
        self.lengths.insert(path.len());
        self.by_key.entry(key(path)).or_insert(index);
    }

    /// Returns the position of the requested header at `path`, matched ignoring case and
    /// separator kind.
    fn find(&self, path: &str) -> Option<usize> {
        if !self.lengths.contains(&path.len()) {
            return None;
        }
        self.by_key.get(&key(path)).copied()
    }
}

/// Finds the parts and alpha tail Entries of the `headers` in one parallel pass over the source.
fn source_members<'a>(
    source: &'a dyn EntrySource,
    headers: &Headers,
) -> Vec<(usize, Member, SourceEntry<'a>)> {
    let owner = |path: &str| {
        let Some(Role::Member(header_path, member)) = role(path) else {
            return None;
        };
        Some((headers.find(header_path)?, member))
    };
    source
        .members(&|path| owner(path).is_some())
        .into_iter()
        .filter_map(|entry| {
            let (header, member) = owner(entry.path())?;
            Some((header, member, entry))
        })
        .collect()
}

/// What an Entry is to texture Conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Role<'p> {
    /// A `.dds` header.
    Header,
    /// An Entry belonging to the header at the given path.
    Member(&'p str, Member),
}

/// What an Entry other than the header is to its texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Member {
    /// Part `N`, `.dds.N`.
    Part(u32),
    /// The alpha tail's header, `.dds.a`.
    AlphaHeader,
    /// Part `N` of the alpha tail, `.dds.Na`.
    AlphaPart(u32),
}

/// Classifies DDS family syntax in the supplied UTF-8 path.
///
/// Physical identity and raw-path losslessness require the owning Archive's evidence.
#[must_use]
pub fn role(path: &str) -> Option<Role<'_>> {
    if has_dds_extension(path) {
        return Some(Role::Header);
    }
    let (header, suffix) = path.rsplit_once('.')?;
    if !has_dds_extension(header) {
        return None;
    }
    if suffix.eq_ignore_ascii_case("a") {
        return Some(Role::Member(header, Member::AlphaHeader));
    }
    let (digits, alpha) = suffix
        .strip_suffix(['a', 'A'])
        .map_or((suffix, false), |digits| (digits, true));
    if digits.is_empty()
        || digits.starts_with('0')
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let number = digits.parse().ok()?;
    let member = if alpha {
        Member::AlphaPart(number)
    } else {
        Member::Part(number)
    };
    Some(Role::Member(header, member))
}

fn has_dds_extension(path: &str) -> bool {
    path.len() > 4
        && path
            .get(path.len() - 4..)
            .is_some_and(|extension| extension.eq_ignore_ascii_case(".dds"))
}

/// Returns the path in lower case with `\` separators, the key two paths collide on.
fn key(path: &str) -> String {
    path.replace('/', "\\").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::{Member, Role, role};

    #[test]
    fn headers_parts_and_alpha_tails_are_recognised() {
        let member = |member| Some(Role::Member("Data\\a.dds", member));
        for (path, expected) in [
            ("Data\\a.dds", Some(Role::Header)),
            ("Data/A.DDS", Some(Role::Header)),
            ("Data\\a.dds.12", member(Member::Part(12))),
            ("Data\\a.dds.a", member(Member::AlphaHeader)),
            ("Data\\a.dds.A", member(Member::AlphaHeader)),
            ("Data\\a.dds.3a", member(Member::AlphaPart(3))),
            ("Data\\a.dds.7A", member(Member::AlphaPart(7))),
        ] {
            assert_eq!(role(path), expected, "{path}");
        }
        for other in [
            "Data\\a.dds.0",
            "Data\\a.dds.0a",
            "Data\\a.dds.01",
            "Data\\a.dds.aa",
            "Data\\a.dds.1b",
            "Data\\a.dds.",
            "Data\\a.ddsx",
            "Data\\a.png.1",
            "Data\\a.dds.99999999999",
            ".dds",
        ] {
            assert_eq!(role(other), None, "{other} is no texture Entry");
        }
    }
}
