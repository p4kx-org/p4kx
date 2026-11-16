// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::slice;

use p4kx_audio::{ActionTarget, HircKind, HircObject, HircValue, SoundSource};

use crate::audio::index::Stinger;
use crate::audio::music_clip_features::MusicClipFeatures;
use crate::audio::resolve_options::{ResolveWork, WorkStop};
use crate::audio::walk_banks::{Via, select_holders};
use crate::audio::{AudioIndex, BreakReason, LanguageChoice};

use super::{FollowedEdge, Target};

pub(super) enum Visit {
    Follow(Target),
    Sound(SoundSource),
    Track(SoundSource, usize, MusicClipFeatures),
    Break(BreakReason, Option<FollowedEdge>),
}

pub(super) struct WalkCursor<'i>(Continuation<'i>);

enum Continuation<'i> {
    Once(Option<Visit>),
    Children(usize),
    Track(usize),
    Stingers(StingerCursor<'i>),
}

impl<'i> WalkCursor<'i> {
    pub(super) fn new(
        object: &HircObject,
        version: u32,
        index: &'i AudioIndex,
        choice: &LanguageChoice,
        work: &ResolveWork<'_>,
    ) -> Result<Self, WorkStop> {
        let continuation = match &object.value {
            HircValue::Event(_)
            | HircValue::Container(_)
            | HircValue::MusicSegment(_)
            | HircValue::MusicSwitch(_)
            | HircValue::MusicPlaylist(_) => Continuation::Children(0),
            HircValue::Action(action) if action.is_trigger() => {
                stinger_continuation(index, choice, action.target_id, work)?
            }
            HircValue::Action(action) => {
                Continuation::Once(action_target(action).map(Visit::Follow))
            }
            HircValue::MusicTrack(_) => Continuation::Track(0),
            HircValue::Sound(source) => Continuation::Once(Some(Visit::Sound(*source))),
            HircValue::ActorMixer(_)
            | HircValue::State(_)
            | HircValue::Attenuation
            | HircValue::Bus
            | HircValue::Plugin
            | HircValue::Modulator
            | HircValue::Malformed(_)
            | HircValue::Raw => {
                Continuation::Once(Some(Visit::Break(unresolved(object, version), None)))
            }
        };
        Ok(Self(continuation))
    }

    pub(super) fn next(
        &mut self,
        object: Option<&HircObject>,
        work: &ResolveWork<'_>,
    ) -> Result<Option<Visit>, WorkStop> {
        match &mut self.0 {
            Continuation::Once(next) => Ok(next.take()),
            Continuation::Children(next) => Ok(object.and_then(|object| child(object, next))),
            Continuation::Track(next) => Ok(object.and_then(|object| track(object, next))),
            Continuation::Stingers(stingers) => stingers.next(work),
        }
    }
}

fn action_target(action: &ActionTarget) -> Option<Target> {
    let (wanted, via) = if action.is_play() {
        (
            None,
            action
                .bank
                .map_or(Via::Local, |bank| Via::Bank(bank.bank_id)),
        )
    } else if action.is_play_event() {
        (Some(HircKind::Event), Via::PlayEvent)
    } else {
        return None;
    };
    Some(Target {
        id: action.target_id,
        wanted,
        via,
        inbound: Some(FollowedEdge::Action),
    })
}

fn child(object: &HircObject, next: &mut usize) -> Option<Visit> {
    let children = match &object.value {
        HircValue::Event(actions) => actions,
        HircValue::Container(container) => &container.children,
        HircValue::MusicSegment(segment) => &segment.node.children,
        HircValue::MusicSwitch(switch) => &switch.node.children,
        HircValue::MusicPlaylist(playlist) => &playlist.node.children,
        _ => return None,
    };
    let ordinal = *next;
    let &id = children.get(ordinal)?;
    *next += 1;
    Some(Visit::Follow(Target {
        id,
        wanted: None,
        via: Via::Local,
        inbound: Some(FollowedEdge::Child(ordinal)),
    }))
}

fn track(object: &HircObject, next: &mut usize) -> Option<Visit> {
    let HircValue::MusicTrack(track) = &object.value else {
        return None;
    };
    let ordinal = *next;
    let &source = track.sources.get(ordinal)?;
    *next += 1;
    Some(Visit::Track(
        source,
        ordinal,
        MusicClipFeatures::observe(track),
    ))
}

struct StingerCursor<'i> {
    trigger: u32,
    candidates: slice::Iter<'i, Stinger>,
    chosen: Vec<u32>,
}

fn stinger_continuation<'i>(
    index: &'i AudioIndex,
    choice: &LanguageChoice,
    trigger: u32,
    work: &ResolveWork<'_>,
) -> Result<Continuation<'i>, WorkStop> {
    let stingers = index.stingers(trigger);
    if stingers.is_empty() {
        return Ok(Continuation::Once(Some(Visit::Break(
            BreakReason::Missing { id: trigger },
            Some(FollowedEdge::Action),
        ))));
    }
    let mut banks = BTreeSet::new();
    for stinger in stingers {
        if work.cancelled() {
            return Err(WorkStop::Cancelled);
        }
        banks.insert(stinger.bank);
    }
    let positions: Vec<_> = banks.into_iter().collect();
    let chosen = select_holders(index, &positions, choice);
    if chosen.is_empty() {
        return Ok(Continuation::Once(Some(Visit::Break(
            choice.no_localisation(),
            Some(FollowedEdge::Action),
        ))));
    }
    Ok(Continuation::Stingers(StingerCursor {
        trigger,
        candidates: stingers.iter(),
        chosen,
    }))
}

impl StingerCursor<'_> {
    fn next(&mut self, work: &ResolveWork<'_>) -> Result<Option<Visit>, WorkStop> {
        for stinger in self.candidates.by_ref() {
            if work.cancelled() {
                return Err(WorkStop::Cancelled);
            }
            if self.chosen.binary_search(&stinger.bank).is_ok() {
                return Ok(Some(Visit::Follow(Target {
                    id: stinger.segment_id,
                    wanted: None,
                    via: Via::At(stinger.bank),
                    inbound: Some(FollowedEdge::Stinger {
                        trigger: self.trigger,
                        declaration: stinger.scheduled_at,
                    }),
                })));
            }
        }
        Ok(None)
    }
}

fn unresolved(object: &HircObject, version: u32) -> BreakReason {
    match &object.value {
        HircValue::Malformed(error)
            if !matches!(
                object.kind,
                HircKind::State
                    | HircKind::ActorMixer
                    | HircKind::Attenuation
                    | HircKind::AudioBus
                    | HircKind::AuxiliaryBus
                    | HircKind::FxShareSet
                    | HircKind::FxCustom
                    | HircKind::AudioDevice
                    | HircKind::Lfo
                    | HircKind::Envelope
                    | HircKind::TimeModulator
            ) =>
        {
            BreakReason::Malformed(error.clone())
        }
        HircValue::Raw
            if matches!(
                object.kind,
                HircKind::Sound
                    | HircKind::RandomSequenceContainer
                    | HircKind::SwitchContainer
                    | HircKind::LayerContainer
                    | HircKind::MusicSegment
                    | HircKind::MusicTrack
                    | HircKind::MusicSwitch
                    | HircKind::MusicPlaylist
            ) =>
        {
            BreakReason::UnknownLayout { version }
        }
        _ => BreakReason::Unsupported,
    }
}
