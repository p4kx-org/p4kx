// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::ConversionError;
use rayon::prelude::*;

use super::resolve_bounded;
use super::resolve_options::ResolveWork;
use super::walk::{self, Found};
use super::walk_banks::{self, Banks, Loaded, Spare};
use super::{
    ArchiveAudio, AudioFileError, AudioIndex, BreakReason, BrokenLink, LanguageChoice, NodeKind,
    PathStep, ResolveOptions, TriggerSounds, WwiseEvent,
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
    /// its fallback language when the first has none. A streamed localised sound is located in
    /// that language's folder of loose WEMs. Where neither language has the sound, the trigger
    /// gets a [`BreakReason::NoLocalisation`] break.
    ///
    /// Each soundbank holding one of the events is read again from the Archive, in parallel, and
    /// dropped once followed. Active referenced banks share their decoded owners. Events of one
    /// start soundbank share the last foreign result. Where a reference names several soundbanks,
    /// one per language, a walk that has no language chosen follows the first in path order
    /// (ADR-0033). Returns `None` when no ATL definition has the trigger.
    #[must_use]
    pub fn resolve_trigger_in(
        &self,
        name: &str,
        choice: &LanguageChoice,
    ) -> Option<TriggerSounds<'_>> {
        self.resolve_with_index(self.index(), name, choice, ResolveOptions::default())
    }

    /// Resolves a trigger and retains the original HIRC byte spans of the visited path steps.
    #[must_use]
    pub fn resolve_trigger_with_path_sources(
        &self,
        name: &str,
        choice: &LanguageChoice,
    ) -> Option<TriggerSounds<'_>> {
        self.resolve_with_index(
            self.index(),
            name,
            choice,
            ResolveOptions::default().with_path_sources(),
        )
    }

    /// Resolves a trigger with optional traversal and cancellation controls.
    ///
    /// A visit budget follows authored events and eligible soundbanks in output order.
    /// Exhaustion yields a partial inspection with a [`BreakReason::TooManyObjects`] diagnostic.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::Cancelled`] if the caller cancels resolution.
    pub fn resolve_trigger_with_options(
        &self,
        name: &str,
        choice: &LanguageChoice,
        options: ResolveOptions<'_>,
    ) -> Result<Option<TriggerSounds<'_>>, ConversionError> {
        let index = if let Some(cancel) = options.cancel {
            self.index_until(cancel).ok_or(ConversionError::Cancelled)?
        } else {
            self.index()
        };
        let sounds = self.resolve_with_index(index, name, choice, options);
        if options.cancelled() {
            return Err(ConversionError::Cancelled);
        }
        Ok(sounds)
    }

    fn resolve_with_index<'i>(
        &self,
        index: &'i AudioIndex,
        name: &str,
        choice: &LanguageChoice,
        options: ResolveOptions<'_>,
    ) -> Option<TriggerSounds<'i>> {
        let trigger = index.trigger(name)?;
        let definition = index.definition_file(trigger);
        let mut sounds = TriggerSounds {
            trigger,
            definition_path: definition.map(|(path, _)| path),
            definition_entry: definition.map(|(_, position)| position),
            choice: choice.clone(),
            variants: Vec::new(),
            breaks: Vec::new(),
        };
        if options.max_visits.is_some() {
            resolve_bounded::walk(self.archive(), index, choice, options, &mut sounds);
        } else {
            self.resolve_parallel(index, choice, options, &mut sounds);
        }
        Some(sounds)
    }

    fn resolve_parallel<'i>(
        &self,
        index: &'i AudioIndex,
        choice: &LanguageChoice,
        options: ResolveOptions<'_>,
        sounds: &mut TriggerSounds<'i>,
    ) {
        let events = sounds.trigger.events();
        let holders: Vec<Vec<u32>> = events
            .iter()
            .take_while(|_| !options.cancelled())
            .map(|event| walk_banks::select_holders(index, index.event_holders(event.id()), choice))
            .collect();
        let mut walks = self.walk_soundbanks(index, events, &holders, choice, options);
        for (number, (event, selected)) in events.iter().zip(holders).enumerate() {
            if options.cancelled() {
                return;
            }
            let path = event_path(event);
            if selected.is_empty() {
                sounds.breaks.push(unselected(index, event, choice, path));
                continue;
            }
            for position in selected {
                let Some(soundbank) = walk_banks::soundbank_at(index, position) else {
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
                        path: path.clone(),
                        path_sources: Box::default(),
                        reason: BreakReason::Unreadable(error.clone()),
                    }),
                    None => {}
                }
            }
        }
    }

    fn walk_soundbanks<'i>(
        &self,
        index: &'i AudioIndex,
        events: &'i [WwiseEvent],
        holders: &[Vec<u32>],
        choice: &LanguageChoice,
        options: ResolveOptions<'_>,
    ) -> Vec<BankWalk<'i>> {
        let mut positions: Vec<u32> = holders.iter().flatten().copied().collect();
        positions.sort_unstable();
        positions.dedup();
        positions
            .par_iter()
            .map(|&position| {
                (
                    position,
                    self.walk_soundbank(index, position, events, holders, choice, options),
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
        options: ResolveOptions<'_>,
    ) -> Result<Vec<(usize, Found<'i>)>, AudioFileError> {
        let mut work = ResolveWork::new(options);
        if work.cancelled() {
            return Ok(Vec::new());
        }
        let home = Loaded::read(self.archive(), index, position)?;
        let mut spare = Spare::default();
        Ok(events
            .iter()
            .zip(holders)
            .enumerate()
            .take_while(|_| !options.cancelled())
            .filter(|(_, (_, selected))| selected.contains(&position))
            .map(|(number, (event, _))| {
                let banks = Banks::new(self.archive(), index, choice, home.clone(), &mut spare);
                (
                    number,
                    walk::event(banks, event, options.path_sources, &mut work),
                )
            })
            .collect())
    }
}

pub(super) fn event_path(event: &WwiseEvent) -> Box<[PathStep]> {
    Box::new([PathStep {
        kind: NodeKind::Event,
        id: event.id(),
    }])
}

pub(super) fn unselected<'i>(
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
        path_sources: Box::default(),
        reason,
    }
}
