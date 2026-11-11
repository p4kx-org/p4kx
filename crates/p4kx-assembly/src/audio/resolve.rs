// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use rayon::prelude::*;

use super::walk::{self, Found};
use super::walk_banks::{Banks, Loaded, Spare, select_holders, soundbank_at};
use super::{
    ArchiveAudio, AudioFileError, AudioIndex, BreakReason, BrokenLink, LanguageChoice,
    MAX_EVENT_OBJECTS, MAX_TRIGGER_OBJECTS, NodeKind, PathStep, TriggerSounds, WwiseEvent,
};

type BankWalk<'i> = (u32, Result<Vec<(usize, Found<'i>)>, AudioFileError>);

impl ArchiveAudio<'_> {
    /// Resolves the audio trigger called `name`, ignoring ASCII case, to its sounds in every
    /// language: see [`resolve_trigger_in`](Self::resolve_trigger_in).
    #[must_use]
    pub fn resolve_trigger(&self, name: &str) -> Option<TriggerSounds<'_>> {
        self.resolve_trigger_in(name, &LanguageChoice::any())
    }

    /// Resolves the audio trigger called `name`, ignoring ASCII case, to its sounds: every
    /// variant its Wwise events lead to through actions and random, sequence, switch and blend
    /// containers, and every broken link.
    ///
    /// A localised event is followed only in the soundbanks of the language `choice` names, or of
    /// its fallback language when the first has none; a streamed localised sound is located in
    /// that language's folder of loose WEMs. Where neither language has the sound, the trigger
    /// gets a [`BreakReason::NoLocalisation`] break.
    ///
    /// Each soundbank holding one of the events is read again from the Archive, in parallel, and
    /// dropped once followed. Active referenced banks share their decoded owners; events of one
    /// start soundbank share the last foreign result. Where a reference names
    /// several soundbanks, one per language, a walk that has no language chosen follows the first
    /// in path order, so [`resolve_trigger`](Self::resolve_trigger) lists the first language only
    /// beyond the soundbank of the event (ADR-0033). Returns `None` when no ATL definition has the
    /// trigger.
    #[must_use]
    pub fn resolve_trigger_in(
        &self,
        name: &str,
        choice: &LanguageChoice,
    ) -> Option<TriggerSounds<'_>> {
        let index = self.index();
        let trigger = index.trigger(name)?;
        let events = trigger.events();
        let holders: Vec<Vec<u32>> = events
            .iter()
            .map(|event| select_holders(index, index.event_holders(event.id()), choice))
            .collect();
        let mut walks = self.walk_soundbanks(index, events, &holders, choice);
        let definition = index.definition_file(trigger);
        let mut sounds = TriggerSounds {
            trigger,
            definition_path: definition.map(|(path, _)| path),
            definition_entry: definition.map(|(_, position)| position),
            choice: choice.clone(),
            variants: Vec::new(),
            breaks: Vec::new(),
        };
        for (number, event) in events.iter().enumerate() {
            let event_step = Box::new([PathStep {
                kind: NodeKind::Event,
                id: event.id(),
            }]);
            let selected = holders.get(number).map_or(&[][..], Vec::as_slice);
            if selected.is_empty() {
                sounds
                    .breaks
                    .push(unselected(index, event, choice, event_step));
                continue;
            }
            for &position in selected {
                let Some(soundbank) = soundbank_at(index, position) else {
                    continue;
                };
                let Ok(slot) = walks.binary_search_by_key(&position, |(at, _)| *at) else {
                    continue;
                };
                match walks.get_mut(slot).map(|(_, walk)| walk) {
                    Some(Ok(found)) => {
                        if let Some((_, found)) = found.iter_mut().find(|(at, _)| *at == number) {
                            sounds.variants.append(&mut found.variants);
                            sounds.breaks.append(&mut found.breaks);
                        }
                    }
                    Some(Err(error)) => sounds.breaks.push(BrokenLink {
                        event,
                        soundbank: Some(soundbank),
                        path: event_step.clone(),
                        reason: BreakReason::Unreadable(error.clone()),
                    }),
                    None => {}
                }
            }
        }
        Some(sounds)
    }

    fn walk_soundbanks<'i>(
        &self,
        index: &'i AudioIndex,
        events: &'i [WwiseEvent],
        holders: &[Vec<u32>],
        choice: &LanguageChoice,
    ) -> Vec<BankWalk<'i>> {
        let mut positions: Vec<u32> = holders.iter().flatten().copied().collect();
        positions.sort_unstable();
        positions.dedup();
        let pairs: usize = holders.iter().map(Vec::len).sum();
        let budget = (MAX_TRIGGER_OBJECTS / pairs.max(1)).clamp(1, MAX_EVENT_OBJECTS);
        positions
            .par_iter()
            .map(|&position| {
                (
                    position,
                    self.walk_soundbank(index, position, events, holders, choice, budget),
                )
            })
            .collect()
    }

    fn walk_soundbank<'i>(
        &self,
        index: &'i AudioIndex,
        position: u32,
        events: &'i [WwiseEvent],
        holders: &[Vec<u32>],
        choice: &LanguageChoice,
        budget: usize,
    ) -> Result<Vec<(usize, Found<'i>)>, AudioFileError> {
        let home = Loaded::read(self.archive(), index, position)?;
        let mut spare = Spare::default();
        Ok(events
            .iter()
            .enumerate()
            .filter(|(number, _)| {
                holders
                    .get(*number)
                    .is_some_and(|selected| selected.contains(&position))
            })
            .map(|(number, event)| {
                let banks = Banks::new(self.archive(), index, choice, home.clone(), &mut spare);
                (number, walk::event(banks, event, budget))
            })
            .collect())
    }
}

fn unselected<'i>(
    index: &AudioIndex,
    event: &'i WwiseEvent,
    choice: &LanguageChoice,
    path: Box<[PathStep]>,
) -> BrokenLink<'i> {
    let reason = if index.event_holders(event.id()).is_empty() {
        BreakReason::NoSoundbank
    } else {
        choice.no_localisation()
    };
    BrokenLink {
        event,
        soundbank: None,
        path,
        reason,
    }
}
