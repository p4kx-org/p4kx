// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::rc::Rc;

#[cfg(test)]
use self::bank_probe::BankProbe;

use p4kx_archive::Archive;
use p4kx_audio::Soundbank;

use super::read::entry_bytes;
use super::{
    AudioFileError, AudioIndex, BreakReason, IndexedSoundbank, LanguageChoice, MAX_SOUNDBANK_LEN,
};

/// A soundbank read from the Archive and parsed, shared by the walks that follow events in it.
#[derive(Debug, Clone)]
pub(crate) struct Loaded<'i> {
    pub(crate) position: u32,
    pub(crate) soundbank: &'i IndexedSoundbank,
    pub(crate) bank: Rc<Soundbank<Vec<u8>>>,
}

impl<'i> Loaded<'i> {
    /// Reads and parses the soundbank at `position` of the index.
    pub(crate) fn read(
        archive: &Archive,
        index: &'i AudioIndex,
        position: u32,
    ) -> Result<Self, AudioFileError> {
        let soundbank = soundbank_at(index, position).ok_or_else(gone)?;
        let entry = archive.entries().get(soundbank.entry).ok_or_else(gone)?;
        let bank = Soundbank::parse(entry_bytes(archive, entry, MAX_SOUNDBANK_LEN)?)?;
        Ok(Self {
            position,
            soundbank,
            bank: Rc::new(bank),
        })
    }
}

/// How a reference names the soundbank that holds its target.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Via {
    /// The reference stays in the soundbank it was found in (a child, a source, a first Event).
    Local,
    /// A Play action names the soundbank by its ID.
    Bank(u32),
    /// A `PlayEvent` action: any soundbank that holds the Event.
    PlayEvent,
    /// A stinger of the music index: the soundbank at this position of the index.
    At(u32),
}

/// A reference the walk does not follow, with the soundbank the break is reported in, or `None`
/// for the soundbank the walk is in.
#[derive(Debug)]
pub(crate) struct Refused<'i> {
    pub(crate) reason: BreakReason,
    pub(crate) bank: Option<&'i IndexedSoundbank>,
}

impl Refused<'_> {
    const fn here(reason: BreakReason) -> Self {
        Self { reason, bank: None }
    }
}

/// The last other soundbank read by the event walks of one start soundbank, kept for the next
/// event, whether it could be read or not.
#[derive(Debug, Default)]
pub(crate) struct Spare<'i> {
    slot: Option<(u32, Result<Loaded<'i>, AudioFileError>)>,
    #[cfg(test)]
    probe: Option<BankProbe>,
}

impl<'i> Spare<'i> {
    fn get(
        &mut self,
        archive: &Archive,
        index: &'i AudioIndex,
        position: u32,
    ) -> Result<Loaded<'i>, AudioFileError> {
        if let Some((at, result)) = &self.slot
            && *at == position
        {
            return result.clone();
        }
        let result = Loaded::read(archive, index, position);
        #[cfg(test)]
        if let Some(probe) = &mut self.probe {
            probe.reads.push(position);
        }
        self.slot = Some((position, result.clone()));
        result
    }
}

/// Indexed bank selection, stable first-foreign preference and per-walk read failures.
#[derive(Debug)]
pub(crate) struct Banks<'w, 'i> {
    archive: &'w Archive,
    pub(crate) index: &'i AudioIndex,
    pub(crate) choice: &'w LanguageChoice,
    pub(crate) home: Loaded<'i>,
    spare: &'w mut Spare<'i>,
    first_foreign: Option<u32>,
    failed: HashMap<u32, AudioFileError>,
}

impl<'w, 'i> Banks<'w, 'i> {
    pub(crate) fn new(
        archive: &'w Archive,
        index: &'i AudioIndex,
        choice: &'w LanguageChoice,
        home: Loaded<'i>,
        spare: &'w mut Spare<'i>,
    ) -> Self {
        Self {
            archive,
            index,
            choice,
            home,
            spare,
            first_foreign: None,
            failed: HashMap::new(),
        }
    }

    /// Selects a referenced bank and shares an admitted active owner before consulting the spare.
    pub(crate) fn enter<'active>(
        &mut self,
        from: u32,
        id: u32,
        via: Via,
        mut active: impl Iterator<Item = &'active Loaded<'i>>,
    ) -> Result<Loaded<'i>, Refused<'i>>
    where
        'i: 'active,
    {
        let position = self.select(from, id, via)?;
        if position == self.home.position {
            return Ok(self.home.clone());
        }
        if let Some(error) = self.failed.get(&position) {
            return Err(Refused {
                reason: BreakReason::Unreadable(error.clone()),
                bank: soundbank_at(self.index, position),
            });
        }
        if let Some(loaded) = active.find(|loaded| loaded.position == position) {
            return Ok(loaded.clone());
        }
        let result = self.spare.get(self.archive, self.index, position);
        if let Err(error) = &result {
            self.failed.insert(position, error.clone());
        }
        result.map_err(|error| Refused {
            reason: BreakReason::Unreadable(error),
            bank: soundbank_at(self.index, position),
        })
    }

    fn select(&mut self, from: u32, id: u32, via: Via) -> Result<u32, Refused<'i>> {
        let candidates = self.candidates(id, via).map_err(Refused::here)?;
        if candidates.contains(&from) {
            return Err(Refused::here(BreakReason::Missing { id }));
        }
        let Some(&wanted) = candidates.first() else {
            return Err(Refused::here(match via {
                Via::Bank(bank_id) => BreakReason::OtherSoundbank { id, bank_id },
                Via::Local | Via::PlayEvent | Via::At(_) => BreakReason::Missing { id },
            }));
        };
        if candidates.contains(&self.home.position) {
            return Ok(self.home.position);
        }
        let position = self
            .first_foreign
            .filter(|taken| candidates.contains(taken))
            .unwrap_or(wanted);
        self.first_foreign.get_or_insert(position);
        Ok(position)
    }
    #[cfg(test)]
    pub(super) fn probe_selection(
        &mut self,
        position: u32,
        object: u32,
        bank: &Rc<Soundbank<Vec<u8>>>,
    ) {
        if let Some(probe) = &mut self.spare.probe {
            probe.selected(position, object, bank);
        }
    }

    fn candidates(&self, id: u32, via: Via) -> Result<Vec<u32>, BreakReason> {
        let positions = match via {
            Via::Local | Via::Bank(0) => return Err(BreakReason::Missing { id }),
            Via::Bank(bank_id) => self.index.bank_positions(bank_id),
            Via::PlayEvent => self.index.event_holders(id),
            Via::At(position) => &[position],
        };
        let selected = select_holders(self.index, positions, self.choice);
        if selected.is_empty() && !positions.is_empty() {
            return Err(self.choice.no_localisation());
        }
        Ok(selected)
    }
}

/// Returns the soundbanks of `holders` an event is followed in: every soundbank that is not
/// localised, and of the localised ones those of the first language of `choice` that has any.
pub(crate) fn select_holders(
    index: &AudioIndex,
    holders: &[u32],
    choice: &LanguageChoice,
) -> Vec<u32> {
    if choice.is_any() {
        return holders.to_vec();
    }
    let language_of =
        |position: u32| soundbank_at(index, position).and_then(IndexedSoundbank::language);
    let mut selected: Vec<u32> = holders
        .iter()
        .copied()
        .filter(|&position| language_of(position).is_none())
        .collect();
    let localised = |wanted: &str| {
        holders
            .iter()
            .copied()
            .filter(|&position| {
                language_of(position).is_some_and(|language| language.eq_ignore_ascii_case(wanted))
            })
            .collect::<Vec<u32>>()
    };
    if let Some(chosen) = choice
        .candidates()
        .map(localised)
        .find(|banks| !banks.is_empty())
    {
        selected.extend(chosen);
        selected.sort_unstable();
    }
    selected
}

pub(crate) fn soundbank_at(index: &AudioIndex, position: u32) -> Option<&IndexedSoundbank> {
    index.soundbanks().get(usize::try_from(position).ok()?)
}

pub(crate) fn gone() -> AudioFileError {
    AudioFileError::Unreadable {
        reason: "the Archive no longer holds the soundbank Entry".to_owned(),
    }
}

#[cfg(test)]
#[path = "bank_probe.rs"]
mod bank_probe;

#[cfg(test)]
#[path = "bank_probe_tests.rs"]
mod probe_tests;
