// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use super::language::{LooseName, bank_language, loose_name};
use super::read::folded_order;
use super::{AudioFileError, MAX_LANGUAGES, MAX_LOOSE_WEMS, WemLocation};

/// A loose WEM Entry of the audio index.
#[derive(Debug)]
struct LooseWem {
    path: Box<str>,
    entry: u32,
    language: u16,
    len: u64,
}

const NO_LANGUAGE: u16 = u16::MAX;

/// A `.wem` Entry below `Data\Sounds\wwise\`, as [`LooseMedia::collect`] reads it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct WemSource<'a> {
    pub(crate) entry: u32,
    pub(crate) path: &'a str,
    pub(crate) len: u64,
}

/// How many loose WEMs and language folders [`LooseMedia::collect`] keeps.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Limits {
    pub(crate) wems: usize,
    pub(crate) languages: usize,
}

impl Limits {
    pub(crate) const DEFAULT: Self = Self {
        wems: MAX_LOOSE_WEMS,
        languages: MAX_LANGUAGES,
    };
}

/// The loose WEM Entries below `Data\Sounds\wwise\`, found by the ID in their file name, and the
/// language folders of the soundbank and WEM Entries there.
#[derive(Debug, Default)]
pub(crate) struct LooseMedia {
    languages: Vec<Box<str>>,
    by_name: HashMap<Box<str>, u16>,
    recent: Option<(Box<str>, u16)>,
    wems: HashMap<u32, Vec<LooseWem>>,
}

impl LooseMedia {
    /// Collects the loose WEMs among `wems`, which are sorted by path, and the languages of them
    /// and of the soundbank folders `bank_paths`.
    ///
    /// Returns the Entries that were left out, the first of each cause: past `limits.wems` WEMs or
    /// `limits.languages` languages further ones are dropped.
    pub(crate) fn collect<'a>(
        wems: impl Iterator<Item = WemSource<'a>>,
        bank_paths: impl Iterator<Item = &'a str>,
        limits: Limits,
    ) -> (Self, Vec<(Box<str>, AudioFileError)>) {
        let mut media = Self::default();
        let mut bank_failure: Option<&str> = None;
        let mut wem_failure: Option<&str> = None;
        for path in bank_paths {
            let Some(name) = bank_language(path) else {
                continue;
            };
            if media.language(name, limits.languages).is_none() {
                bank_failure.get_or_insert(path);
            }
        }
        let mut count = 0_usize;
        for source in wems {
            let Some(LooseName { id, language, .. }) = loose_name(source.path) else {
                continue;
            };
            let language = language.map_or(Some(NO_LANGUAGE), |name| {
                media.language(name, limits.languages)
            });
            let Some(language) = language.filter(|_| count < limits.wems) else {
                wem_failure.get_or_insert(source.path);
                continue;
            };
            count += 1;
            media.wems.entry(id).or_default().push(LooseWem {
                path: source.path.into(),
                entry: source.entry,
                language,
                len: source.len,
            });
        }
        media.sort_languages();
        let failures = [bank_failure, wem_failure]
            .into_iter()
            .flatten()
            .map(|path| (path.into(), AudioFileError::TooManyMedia))
            .collect();
        (media, failures)
    }

    fn sort_languages(&mut self) {
        let mut order: Vec<usize> = (0..self.languages.len()).collect();
        order.sort_by(|&a, &b| {
            let name = |at: usize| self.languages.get(at).map_or("", |name| &**name);
            folded_order(name(a), name(b))
        });
        let mut renumbered = vec![NO_LANGUAGE; order.len()];
        for (new, &old) in order.iter().enumerate() {
            if let Some(slot) = renumbered.get_mut(old) {
                *slot = u16::try_from(new).unwrap_or(NO_LANGUAGE);
            }
        }
        for wem in self.wems.values_mut().flatten() {
            if let Some(&new) = renumbered.get(usize::from(wem.language)) {
                wem.language = new;
            }
        }
        let mut sorted: Vec<Option<Box<str>>> = std::mem::take(&mut self.languages)
            .into_iter()
            .map(Some)
            .collect();
        self.languages = order
            .iter()
            .filter_map(|&old| sorted.get_mut(old)?.take())
            .collect();
        self.by_name = self
            .languages
            .iter()
            .enumerate()
            .filter_map(|(at, name)| {
                Some((name.to_ascii_lowercase().into(), u16::try_from(at).ok()?))
            })
            .collect();
        self.recent = None;
    }

    fn language(&mut self, name: &str, limit: usize) -> Option<u16> {
        if let Some((last, at)) = &self.recent
            && last.eq_ignore_ascii_case(name)
        {
            return Some(*at);
        }
        let folded = name.to_ascii_lowercase();
        let at = if let Some(&at) = self.by_name.get(folded.as_str()) {
            at
        } else {
            if self.languages.len() >= limit {
                return None;
            }
            let at = u16::try_from(self.languages.len()).ok()?;
            self.languages.push(name.into());
            self.by_name.insert(folded.into(), at);
            at
        };
        self.recent = Some((name.into(), at));
        Some(at)
    }

    pub(crate) fn languages(&self) -> &[Box<str>] {
        &self.languages
    }

    pub(crate) fn language_of(&self, name: &str) -> Option<&str> {
        let at = *self.by_name.get(name.to_ascii_lowercase().as_str())?;
        self.languages.get(usize::from(at)).map(|known| &**known)
    }

    /// Returns the loose WEMs with the ID `id`, in path order, with their language folder.
    pub(crate) fn find(&self, id: u32) -> impl Iterator<Item = (Option<&str>, WemLocation<'_>)> {
        self.wems.get(&id).into_iter().flatten().map(|wem| {
            let language = self
                .languages
                .get(usize::from(wem.language))
                .map(|name| &**name);
            (
                language,
                WemLocation::Entry {
                    path: &wem.path,
                    entry: wem.entry,
                    language,
                    len: wem.len,
                },
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wem(entry: u32, path: &str) -> WemSource<'_> {
        WemSource {
            entry,
            path,
            len: 1,
        }
    }

    fn collect<'a>(
        wems: &[WemSource<'a>],
        banks: &[&'a str],
        limits: Limits,
    ) -> (LooseMedia, Vec<(Box<str>, AudioFileError)>) {
        LooseMedia::collect(wems.iter().copied(), banks.iter().copied(), limits)
    }

    fn entries(media: &LooseMedia, id: u32) -> Vec<(u32, Option<String>)> {
        media
            .find(id)
            .filter_map(|(language, location)| match location {
                WemLocation::Entry { entry, .. } => Some((entry, language.map(str::to_owned))),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_same_id_in_two_folders_is_found_in_path_order() {
        let wems = [
            wem(5, "Data\\Sounds\\wwise\\77.wem"),
            wem(3, "Data\\Sounds\\wwise\\Media\\77.wem"),
        ];
        let (media, failures) = collect(&wems, &[], Limits::DEFAULT);
        assert_eq!(failures.len(), 0);
        assert_eq!(entries(&media, 77), vec![(5, None), (3, None)]);
    }

    #[test]
    fn a_leading_zero_name_is_the_number_and_a_name_above_u32_is_no_loose_wem() {
        let wems = [
            wem(1, "Data\\Sounds\\wwise\\Media\\0012.wem"),
            wem(2, "Data\\Sounds\\wwise\\Media\\4294967295.wem"),
            wem(3, "Data\\Sounds\\wwise\\Media\\4294967296.wem"),
            wem(4, "Data\\Sounds\\wwise\\Media\\99999999999999999999999.wem"),
        ];
        let (media, failures) = collect(&wems, &[], Limits::DEFAULT);
        assert_eq!(failures.len(), 0);
        assert_eq!(entries(&media, 12), vec![(1, None)]);
        assert_eq!(entries(&media, u32::MAX), vec![(2, None)]);
        assert_eq!(media.wems.len(), 2);
    }

    #[test]
    fn wems_past_the_cap_are_dropped_and_the_first_is_reported() {
        let wems = [
            wem(1, "Data\\Sounds\\wwise\\Media\\1.wem"),
            wem(2, "Data\\Sounds\\wwise\\Media\\2.wem"),
            wem(3, "Data\\Sounds\\wwise\\Media\\3.wem"),
            wem(4, "Data\\Sounds\\wwise\\Media\\4.wem"),
        ];
        let limits = Limits {
            wems: 2,
            ..Limits::DEFAULT
        };
        let (media, failures) = collect(&wems, &[], limits);
        assert_eq!(entries(&media, 2), vec![(2, None)]);
        assert_eq!(entries(&media, 3).len(), 0);
        assert_eq!(failures.len(), 1);
        assert_eq!(&*failures[0].0, "Data\\Sounds\\wwise\\Media\\3.wem");
        assert!(matches!(failures[0].1, AudioFileError::TooManyMedia));
    }

    #[test]
    fn languages_past_the_cap_of_wems_and_of_banks_are_reported() {
        let wems = [
            wem(1, "Data\\Sounds\\wwise\\Media\\Aa\\1.wem"),
            wem(2, "Data\\Sounds\\wwise\\Media\\Bb\\2.wem"),
        ];
        let banks = [
            "Data\\Sounds\\wwise\\Cc\\x.bnk",
            "Data\\Sounds\\wwise\\Dd\\y.bnk",
        ];
        let limits = Limits {
            languages: 1,
            ..Limits::DEFAULT
        };
        let (media, failures) = collect(&wems, &banks, limits);
        assert_eq!(media.languages().len(), 1);
        let paths: Vec<&str> = failures.iter().map(|(path, _)| &**path).collect();
        assert_eq!(
            paths,
            [
                "Data\\Sounds\\wwise\\Dd\\y.bnk",
                "Data\\Sounds\\wwise\\Media\\Aa\\1.wem"
            ]
        );
    }

    #[test]
    fn a_language_is_found_ignoring_case_and_sorted() {
        let wems = [
            wem(1, "Data\\Sounds\\wwise\\Media\\german\\1.wem"),
            wem(2, "Data\\Sounds\\wwise\\Media\\English(US)\\2.wem"),
            wem(3, "Data\\Sounds\\wwise\\Media\\German\\3.wem"),
        ];
        let (media, _) = collect(&wems, &[], Limits::DEFAULT);
        let names: Vec<&str> = media.languages().iter().map(|name| &**name).collect();
        assert_eq!(names, ["English(US)", "german"]);
        assert_eq!(media.language_of("GERMAN"), Some("german"));
        assert_eq!(entries(&media, 3), vec![(3, Some("german".to_owned()))]);
    }
}
