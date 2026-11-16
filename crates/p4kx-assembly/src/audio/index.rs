// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_archive::{Archive, Entry, ObservedEntry};
use p4kx_extraction::CancelToken;

use super::banks::{BankStinger, StingerDeclaration};
use super::event_sources::BankEventIndex;
use super::index_read::{self, BankRead};
use super::language::SOUND_FOLDER;
use super::media::{BankPath, Limits, LooseMedia, WemSource};
use super::read::{folded_order, has_extension, starts_with_folder};
use super::{
    AtlDefinitions, AtlTrigger, AudioIssue, DEFAULT_LANGUAGE, IndexedSoundbank, WemLocation,
};

/// The audio data of one Archive: every ATL trigger with its Wwise events, and for every Wwise
/// event ID the soundbanks whose `HIRC` section holds the event.
///
/// [`AudioIndex::build`] reads the ATL definitions below [`super::ATL_FOLDER`] and every `.bnk` Entry
/// once, in parallel; an Entry that cannot be read is left out and kept as an [`AudioIssue`].
/// Triggers are looked up by name ignoring ASCII case, events by ID; when several definitions
/// name the same trigger, the first by Entry path wins.
#[derive(Debug, Default)]
pub struct AudioIndex {
    definitions: AtlDefinitions,
    soundbanks: Vec<IndexedSoundbank>,
    events: HashMap<u32, Box<[u32]>>,
    pub(super) event_sources: Vec<Option<BankEventIndex>>,
    bank_ids: HashMap<u32, Box<[u32]>>,
    stingers: HashMap<u32, Box<[Stinger]>>,
    media: HashMap<u32, Box<[MediaSpot]>>,
    loose: LooseMedia,
    issues: Vec<AudioIssue>,
}

impl AudioIndex {
    /// Reads the ATL definitions and soundbanks of `archive` into an index.
    #[must_use]
    pub fn build(archive: &Archive) -> Self {
        Self::build_until(archive, &CancelToken::new()).unwrap_or_default()
    }

    /// Reads the ATL definitions and soundbanks of `archive` into an index like
    /// [`build`](Self::build), stopping between Entries once `cancel` is cancelled; returns `None`
    /// then.
    #[must_use]
    pub fn build_until(archive: &Archive, cancel: &CancelToken) -> Option<Self> {
        let mut bank_entries: Vec<(usize, &Entry)> = archive
            .entries()
            .iter()
            .enumerate()
            .filter(|(_, entry)| has_extension(entry.path(), "bnk"))
            .collect();
        let mut wem_entries: Vec<WemSource<'_>> = archive
            .entries()
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                starts_with_folder(entry.path(), SOUND_FOLDER) && has_extension(entry.path(), "wem")
            })
            .map(|(position, entry)| WemSource {
                entry: u32::try_from(position).unwrap_or(u32::MAX),
                path: entry.path(),
                len: entry.uncompressed_size(),
            })
            .collect();
        bank_entries.sort_by(|(_, a), (_, b)| folded_order(a.path(), b.path()));
        wem_entries.sort_by(|a, b| folded_order(a.path, b.path));
        let mut definitions = AtlDefinitions::build_until(archive, cancel)?;
        let soundbanks = index_read::read_soundbanks(archive, &bank_entries, cancel)?;
        let issues = definitions.take_issues();
        let mut index = Self {
            definitions,
            issues,
            ..Self::default()
        };
        index.add_soundbanks(archive, &bank_entries, soundbanks);
        let (loose, failures) = LooseMedia::collect(
            wem_entries.iter().copied(),
            bank_entries.iter().map(|&(position, entry)| BankPath {
                path: entry.path(),
                entry: position,
            }),
            Limits::DEFAULT,
        );
        index.loose = loose;
        index.issues.extend(
            failures
                .into_iter()
                .map(|(path, error)| AudioIssue { path, error }),
        );
        Some(index)
    }

    fn add_soundbanks(
        &mut self,
        archive: &Archive,
        entries: &[(usize, &Entry)],
        soundbanks: Vec<BankRead>,
    ) {
        let mut events: HashMap<u32, Vec<u32>> = HashMap::new();
        let mut media: HashMap<u32, Vec<MediaSpot>> = HashMap::new();
        let mut bank_ids: HashMap<u32, Vec<u32>> = HashMap::new();
        let mut stingers = StingerList::default();
        for ((_, entry), soundbank) in entries.iter().zip(soundbanks) {
            match soundbank {
                Ok(contents) => {
                    let position = u32::try_from(self.soundbanks.len()).unwrap_or(u32::MAX);
                    bank_ids
                        .entry(contents.soundbank.id())
                        .or_default()
                        .push(position);
                    self.soundbanks.push(contents.soundbank);
                    stingers.add(position, &contents.stingers);
                    for declaration in &contents.events {
                        let holders = events.entry(declaration.id).or_default();
                        if holders.last() != Some(&position) {
                            holders.push(position);
                        }
                    }
                    let source = archive.observed_entry(entry).map(|entry| {
                        BankEventIndex::new(entry, contents.event_version, contents.events)
                    });
                    self.event_sources.push(source);
                    for wem in contents.media {
                        media.entry(wem.id).or_default().push(MediaSpot {
                            soundbank: position,
                            offset: wem.offset,
                            len: wem.len,
                            triplet_offset: wem.triplet_offset,
                        });
                    }
                }
                Err(error) => self.issues.push(AudioIssue {
                    path: entry.path().into(),
                    error,
                }),
            }
        }
        self.events = boxed(events);
        self.bank_ids = boxed(bank_ids);
        self.stingers = boxed(stingers.found);
        self.media = boxed(media);
    }

    /// Returns the trigger called `name`, ignoring ASCII case, or `None` when no ATL definition
    /// has it.
    #[must_use]
    pub fn trigger(&self, name: &str) -> Option<&AtlTrigger> {
        self.definitions.trigger(name)
    }

    /// Returns every trigger, in the order of their definitions' Entry paths.
    #[must_use]
    pub fn triggers(&self) -> &[AtlTrigger] {
        self.definitions.triggers()
    }

    /// Returns the path of the ATL definition Entry that defines `trigger`, or `None` for a
    /// trigger of another index.
    #[must_use]
    pub fn definition_path(&self, trigger: &AtlTrigger) -> Option<&str> {
        self.definitions.definition_path(trigger)
    }

    /// Returns the Archive position of the Entry that defines this admitted trigger.
    #[must_use]
    pub fn definition_entry_position(&self, trigger: &AtlTrigger) -> Option<usize> {
        self.definitions.definition_entry_position(trigger)
    }

    /// Returns the physically bound definition Entry behind an admitted trigger.
    #[must_use]
    pub fn definition_source<'archive>(
        &self,
        archive: &'archive Archive,
        trigger: &AtlTrigger,
    ) -> Option<ObservedEntry<'archive>> {
        self.definitions.definition_source(archive, trigger)
    }

    pub(super) fn definition_file(&self, trigger: &AtlTrigger) -> Option<(&str, usize)> {
        self.definitions.definition_file(trigger)
    }

    /// Returns every soundbank that was parsed, in path order.
    #[must_use]
    pub fn soundbanks(&self) -> &[IndexedSoundbank] {
        &self.soundbanks
    }

    /// Returns the soundbanks whose `HIRC` section holds the Event object `id`, in path order.
    pub fn event_soundbanks(&self, id: u32) -> impl Iterator<Item = &IndexedSoundbank> {
        self.soundbanks_at(self.event_holders(id))
    }

    pub(crate) fn soundbanks_at<'i>(
        &'i self,
        holders: &'i [u32],
    ) -> impl Iterator<Item = &'i IndexedSoundbank> + use<'i> {
        holders
            .iter()
            .filter_map(|&position| self.soundbanks.get(usize::try_from(position).ok()?))
    }

    /// Returns the Entries that were left out, with the reason, in path order per kind.
    #[must_use]
    pub fn issues(&self) -> &[AudioIssue] {
        &self.issues
    }

    /// Returns where the soundbanks embed the WEM `id` in their `DATA` sections, in path order.
    pub fn embedded_wem(&self, id: u32) -> impl Iterator<Item = WemLocation<'_>> {
        self.embedded_wem_with_triplet_offsets(id)
            .map(|(location, _)| location)
    }

    /// Returns embedded WEM locations with their physical bank-relative DIDX triplet offsets.
    pub fn embedded_wem_with_triplet_offsets(
        &self,
        id: u32,
    ) -> impl Iterator<Item = (WemLocation<'_>, u64)> {
        self.media
            .get(&id)
            .map_or(&[][..], |spots| spots)
            .iter()
            .filter_map(|spot| {
                Some((
                    WemLocation::Soundbank {
                        soundbank: self.soundbanks.get(usize::try_from(spot.soundbank).ok()?)?,
                        offset: spot.offset,
                        len: spot.len,
                    },
                    spot.triplet_offset,
                ))
            })
    }

    /// Returns the language folders below `Data\Sounds\wwise\` that hold soundbanks or loose WEMs,
    /// e.g. `English(US)`, in name order ignoring ASCII case.
    #[must_use]
    pub fn languages(&self) -> &[Box<str>] {
        self.loose.languages()
    }

    /// Returns the language folder called `name`, ignoring ASCII case, spelled as the Archive
    /// spells it, or `None` when the Archive holds none.
    #[must_use]
    pub fn language(&self, name: &str) -> Option<&str> {
        self.loose.language_of(name)
    }

    /// Returns the original Entry supplying the displayed spelling of a language folder.
    #[must_use]
    pub fn language_entry_position(&self, name: &str) -> Option<usize> {
        self.loose.language_entry_position(name)
    }

    /// Returns the language folder `English(US)`, the game's default, or the first language folder
    /// when the Archive holds none of that name, or `None` when it holds no language folder.
    #[must_use]
    pub fn default_language(&self) -> Option<&str> {
        self.language(DEFAULT_LANGUAGE)
            .or_else(|| self.languages().first().map(|name| &**name))
    }

    /// Returns the loose WEM Entries below `Data\Sounds\wwise\` named `<id>.wem`, in path order.
    pub fn loose_wem(&self, id: u32) -> impl Iterator<Item = WemLocation<'_>> {
        self.loose.find(id).map(|(_, location)| location)
    }

    pub(crate) const fn loose(&self) -> &LooseMedia {
        &self.loose
    }

    pub(crate) fn event_holders(&self, id: u32) -> &[u32] {
        self.events.get(&id).map_or(&[], |holders| holders)
    }

    pub(crate) fn bank_positions(&self, id: u32) -> &[u32] {
        self.bank_ids.get(&id).map_or(&[], |positions| positions)
    }

    pub(super) fn stingers(&self, trigger: u32) -> &[Stinger] {
        self.stingers.get(&trigger).map_or(&[], |found| found)
    }
}

fn boxed<T>(lists: HashMap<u32, Vec<T>>) -> HashMap<u32, Box<[T]>> {
    lists
        .into_iter()
        .map(|(id, list)| (id, list.into_boxed_slice()))
        .collect()
}

#[derive(Default)]
struct StingerList {
    found: HashMap<u32, Vec<Stinger>>,
    seen: HashSet<(u32, u32, u32)>,
}

impl StingerList {
    fn add(&mut self, bank: u32, stingers: &[BankStinger]) {
        for stinger in stingers {
            let trigger = stinger.ids.trigger_id;
            let segment = stinger.ids.segment_id;
            if self.seen.insert((trigger, bank, segment)) {
                self.found.entry(trigger).or_default().push(Stinger {
                    bank,
                    segment_id: segment,
                    scheduled_at: stinger.scheduled_at,
                });
            }
        }
    }
}

/// The segment a stinger plays and the soundbank, by position, whose music node declares it.
#[derive(Debug)]
pub(super) struct Stinger {
    pub(super) bank: u32,
    pub(super) segment_id: u32,
    pub(super) scheduled_at: Option<StingerDeclaration>,
}

/// Where a soundbank embeds a WEM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MediaSpot {
    soundbank: u32,
    offset: u64,
    len: u64,
    triplet_offset: u64,
}

#[cfg(test)]
mod tests {
    use p4kx_audio::MusicStinger;

    use super::*;

    fn stinger(trigger_id: u32, segment_id: u32) -> BankStinger {
        BankStinger {
            ids: MusicStinger {
                trigger_id,
                segment_id,
            },
            scheduled_at: None,
        }
    }

    #[test]
    fn all_distinct_stinger_tuples_survive_multiple_banks() {
        let crafted: Vec<BankStinger> = (0..524_288).map(|number| stinger(number, 1)).collect();
        let mut list = StingerList::default();
        list.add(0, &crafted);
        list.add(1, &crafted);
        assert_eq!(
            list.seen.len(),
            2 * crafted.len(),
            "every distinct bank tuple is retained"
        );
        assert_eq!(
            list.found.len(),
            crafted.len(),
            "all original trigger identities survive"
        );
        assert!(
            list.found.values().all(|found| found.len() == 2),
            "each trigger keeps both distinct bank declarations"
        );
    }

    #[test]
    fn a_stinger_repeated_in_one_trigger_is_kept_once() {
        let repeated = vec![stinger(7, 1); 100_000];
        let mut list = StingerList::default();
        list.add(3, &repeated);
        list.add(3, &repeated);
        list.add(4, &[stinger(7, 1)]);
        let found = list.found.get(&7).map_or(0, Vec::len);
        assert_eq!(found, 2);
    }
}
