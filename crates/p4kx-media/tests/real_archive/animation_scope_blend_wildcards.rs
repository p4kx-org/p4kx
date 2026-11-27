// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Entry, archive_animation_path};

use super::sources::Family;

pub(super) struct Pattern<'source> {
    directory: String,
    filename_prefix: &'source str,
    extension: &'source str,
    recursive: bool,
}

impl<'source> Pattern<'source> {
    pub(super) fn parse(name: &str, path: &'source str, folder: Option<&str>) -> Option<Self> {
        let alias_prefix = name.strip_suffix('*')?;
        if name.starts_with(['$', '#']) || alias_prefix.contains(['*', '?', '/', '\\']) {
            return None;
        }
        let mut parts = path.split(['/', '\\']).collect::<Vec<_>>();
        let (stem, extension) = parts.pop()?.rsplit_once('.')?;
        let filename_prefix = stem.strip_suffix('*')?;
        if filename_prefix.contains(['*', '?']) || blend_family(extension).is_none() {
            return None;
        }
        let recursive = parts.last() == Some(&"*");
        if recursive {
            parts.pop();
        }
        let needs_folder = !parts.first().is_some_and(|first| rooted(first));
        let mut candidate = String::new();
        if needs_folder {
            let folder = folder?.trim_end_matches(['/', '\\']);
            if !folder.split(['/', '\\']).next().is_some_and(rooted) {
                return None;
            }
            candidate.push_str(folder);
            candidate.push('/');
        }
        for part in parts {
            candidate.push_str(part);
            candidate.push('/');
        }
        candidate.push_str(filename_prefix);
        candidate.push('.');
        candidate.push_str(extension);
        let mut directory = archive_animation_path(&candidate)?;
        directory.truncate(directory.rfind('/')? + 1);
        directory.make_ascii_lowercase();
        Some(Self {
            directory,
            filename_prefix,
            extension,
            recursive,
        })
    }

    fn selects(&self, normalized: &str) -> bool {
        let Some(relative) = normalized.strip_prefix(&self.directory) else {
            return false;
        };
        if !self.recursive && relative.contains('/') {
            return false;
        }
        relative
            .rsplit('/')
            .next()
            .and_then(|file| file.rsplit_once('.'))
            .is_some_and(|(stem, extension)| {
                extension.eq_ignore_ascii_case(self.extension)
                    && stem
                        .as_bytes()
                        .get(..self.filename_prefix.len())
                        .is_some_and(|prefix| {
                            prefix.eq_ignore_ascii_case(self.filename_prefix.as_bytes())
                        })
            })
    }
}

const fn rooted(value: &str) -> bool {
    value.eq_ignore_ascii_case("Data") || value.eq_ignore_ascii_case("Animations")
}

pub(super) const fn blend_family(extension: &str) -> Option<Family> {
    if extension.eq_ignore_ascii_case("bspace") {
        Some(Family::BlendSpace)
    } else if extension.eq_ignore_ascii_case("comb") {
        Some(Family::CombinedSpace)
    } else {
        None
    }
}

pub(super) struct Directory<'archive>(Vec<Source<'archive>>);

struct Source<'archive> {
    entry: &'archive Entry,
    path: String,
    family: Family,
}

impl<'archive> Directory<'archive> {
    pub(super) fn read(archive: &'archive Archive) -> Self {
        let mut sources = archive
            .entries()
            .iter()
            .filter(|entry| archive.raw_path(entry.index()).is_none())
            .filter_map(Source::indexed)
            .collect::<Vec<_>>();
        sources.sort_unstable_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.entry.index().cmp(&right.entry.index()))
        });
        Self(sources)
    }

    pub(super) fn matches<'index>(
        &'index self,
        pattern: &'index Pattern<'_>,
    ) -> impl Iterator<Item = (usize, Family)> + 'index {
        let first = self
            .0
            .partition_point(|source| source.path < pattern.directory);
        self.0
            .get(first..)
            .into_iter()
            .flatten()
            .take_while(|source| source.path.starts_with(&pattern.directory))
            .filter(|source| pattern.selects(&source.path))
            .map(|source| (source.entry.index(), source.family))
    }
}

impl<'archive> Source<'archive> {
    fn indexed(entry: &'archive Entry) -> Option<Self> {
        let (root, _) = entry.path().split_once(['/', '\\'])?;
        if !root.eq_ignore_ascii_case("Data") {
            return None;
        }
        let file = entry.path().rsplit(['/', '\\']).next()?;
        let (stem, extension) = file.rsplit_once('.')?;
        if stem.is_empty() {
            return None;
        }
        Some(Self {
            entry,
            path: archive_animation_path(entry.path())?.to_ascii_lowercase(),
            family: blend_family(extension)?,
        })
    }
}
