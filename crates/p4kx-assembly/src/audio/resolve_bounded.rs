// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::Archive;

use super::resolve;
use super::resolve_options::{ResolveWork, WorkStop};
use super::walk;
use super::walk_banks::{self, Banks, Loaded, Spare};
use super::{
    AudioFileError, AudioIndex, BreakReason, BrokenLink, IndexedSoundbank, LanguageChoice,
    ResolveOptions, TriggerSounds, WwiseEvent,
};

struct CachedHome<'i> {
    position: u32,
    bank: Result<Loaded<'i>, AudioFileError>,
    spare: Spare<'i>,
}

impl<'i> CachedHome<'i> {
    fn read_or_reuse(
        cached: Option<Self>,
        archive: &Archive,
        index: &'i AudioIndex,
        position: u32,
    ) -> Self {
        cached
            .filter(|home| home.position == position)
            .unwrap_or_else(|| Self {
                position,
                bank: Loaded::read(archive, index, position),
                spare: Spare::default(),
            })
    }
}

/// Walks one budgeted trigger in output order, keeping just the last home bank and its spare.
pub(super) fn walk<'i>(
    archive: &Archive,
    index: &'i AudioIndex,
    choice: &LanguageChoice,
    options: ResolveOptions<'_>,
    sounds: &mut TriggerSounds<'i>,
) {
    let mut work = ResolveWork::new(options);
    let mut cached: Option<CachedHome<'i>> = None;
    for event in sounds.trigger.events() {
        if work.cancelled() {
            return;
        }
        let holders = walk_banks::select_holders(index, index.event_holders(event.id()), choice);
        if holders.is_empty() {
            sounds.breaks.push(resolve::unselected(
                index,
                event,
                choice,
                resolve::event_path(event),
            ));
            continue;
        }
        for position in holders {
            let Some(bank) = walk_banks::soundbank_at(index, position) else {
                continue;
            };
            if let Err(stop) = work.admit() {
                if let WorkStop::Budget { limit } = stop {
                    let reason = BreakReason::TooManyObjects { limit };
                    sounds.breaks.push(event_break(event, bank, reason));
                }
                return;
            }
            let mut home = CachedHome::read_or_reuse(cached.take(), archive, index, position);
            match &home.bank {
                Ok(loaded) => {
                    let banks = Banks::new(archive, index, choice, loaded.clone(), &mut home.spare);
                    let mut found = walk::event(banks, event, options.path_sources, &mut work);
                    sounds.variants.append(&mut found.variants);
                    sounds.breaks.append(&mut found.breaks);
                    if work.exhausted_budget() || work.cancelled() {
                        return;
                    }
                }
                Err(error) => {
                    if work.spend().is_err() {
                        return;
                    }
                    let reason = BreakReason::Unreadable(error.clone());
                    sounds.breaks.push(event_break(event, bank, reason));
                }
            }
            cached = Some(home);
        }
    }
}

fn event_break<'i>(
    event: &'i WwiseEvent,
    bank: &'i IndexedSoundbank,
    reason: BreakReason,
) -> BrokenLink<'i> {
    BrokenLink {
        event,
        soundbank: Some(bank),
        path: resolve::event_path(event),
        path_sources: Box::default(),
        reason,
    }
}
