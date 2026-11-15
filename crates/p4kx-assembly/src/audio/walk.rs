// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_audio::{HircKind, HircObject, HircValue, SoundSource};

use super::hirc_path_source::{HircInboundSource, PathSourceMode};
use super::locate::locate;
use super::music_clip_features::MusicClipFeatures;
use super::playback_features::PlaybackFeatures;
use super::resolve_options::{ResolveWork, WorkStop};
use super::walk_banks::{Banks, Loaded, Via, find_object};
use super::{BreakReason, BrokenLink, HircPathSource, IndexedSoundbank, NodeKind, PathStep};
use super::{SoundVariant, WwiseEvent};

#[path = "walk_children.rs"]
mod children;
#[path = "walk_cursor.rs"]
mod cursor;

use self::children::FollowedEdge;
use self::cursor::{Visit, WalkCursor};

/// The variants and broken links one Wwise event leads to in one soundbank.
#[derive(Debug, Default)]
pub(crate) struct Found<'i> {
    pub(crate) variants: Vec<SoundVariant<'i>>,
    pub(crate) breaks: Vec<BrokenLink<'i>>,
}

/// Follows the Wwise event `event` through the `HIRC` section of the start soundbank of `banks`
/// to its sounds through indexed bank references.
pub(super) fn event<'i>(
    banks: Banks<'_, 'i>,
    event: &'i WwiseEvent,
    path_sources: PathSourceMode,
    work: &mut ResolveWork<'_>,
) -> Found<'i> {
    let mut walk = Walk {
        banks,
        event,
        path: Vec::new(),
        active: HashSet::new(),
        shared_path_prefix: 0,
        path_sources,
        found: Found::default(),
    };
    match walk.run(work) {
        Ok(()) => {}
        Err(WorkStop::Budget { limit }) => walk.broke(BreakReason::TooManyObjects { limit }),
        Err(WorkStop::Cancelled) => return Found::default(),
    }
    walk.found
}

#[derive(Clone, Copy)]
struct Target {
    id: u32,
    wanted: Option<HircKind>,
    via: Via,
    inbound: Option<FollowedEdge>,
}

#[derive(PartialEq, Eq, Hash)]
struct ActiveObject {
    position: u32,
    id: u32,
}

struct Frame<'i> {
    owner: Loaded<'i>,
    object_number: usize,
    step: PathStep,
    source: Option<HircPathSource>,
    playback_features: PlaybackFeatures,
    cursor: WalkCursor<'i>,
}

impl Frame<'_> {
    fn object(&self) -> Option<&HircObject> {
        self.owner.bank.objects().get(self.object_number)
    }

    const fn active_object(&self) -> ActiveObject {
        ActiveObject {
            position: self.owner.position,
            id: self.step.id,
        }
    }
}

struct Walk<'w, 'i> {
    banks: Banks<'w, 'i>,
    event: &'i WwiseEvent,
    path: Vec<Frame<'i>>,
    active: HashSet<ActiveObject>,
    shared_path_prefix: usize,
    path_sources: PathSourceMode,
    found: Found<'i>,
}

impl<'i> Walk<'_, 'i> {
    fn run(&mut self, work: &mut ResolveWork<'_>) -> Result<(), WorkStop> {
        self.reach(
            Target {
                id: self.event.id(),
                wanted: Some(HircKind::Event),
                via: Via::Local,
                inbound: None,
            },
            work,
        )?;
        while let Some(frame) = self.path.last_mut() {
            if work.cancelled() {
                return Err(WorkStop::Cancelled);
            }
            let object = frame.owner.bank.objects().get(frame.object_number);
            match frame.cursor.next(object, work)? {
                Some(Visit::Follow(target)) => self.reach(target, work)?,
                Some(Visit::Sound(source)) => self.sound(source, MusicClipFeatures::default()),
                Some(Visit::Track(source, features)) => {
                    work.spend()?;
                    self.sound(source, features);
                }
                Some(Visit::Break(reason)) => self.broke(reason),
                None => self.leave(),
            }
        }
        Ok(())
    }

    fn reach(&mut self, target: Target, work: &mut ResolveWork<'_>) -> Result<(), WorkStop> {
        work.spend()?;
        let current = self.current();
        let from = current.position;
        let in_current = match target.via {
            Via::At(position) => position == from,
            Via::Local | Via::Bank(_) | Via::PlayEvent => true,
        };
        let owner = if in_current && find_object(&current.bank, target.id, target.wanted).is_some()
        {
            current.clone()
        } else {
            match self.banks.enter(
                from,
                target.id,
                target.via,
                self.path.iter().map(|frame| &frame.owner),
            ) {
                Ok(owner) => owner,
                Err(refused) => {
                    self.broke_in(refused.bank, refused.reason);
                    return Ok(());
                }
            }
        };
        self.enter(owner, target, work)
    }

    fn enter(
        &mut self,
        owner: Loaded<'i>,
        target: Target,
        work: &ResolveWork<'_>,
    ) -> Result<(), WorkStop> {
        let Some(object) = find_object(&owner.bank, target.id, target.wanted) else {
            self.broke(BreakReason::Missing { id: target.id });
            return Ok(());
        };
        #[cfg(test)]
        self.banks
            .probe_selection(owner.position, object.id, &owner.bank);
        if !self.active.insert(ActiveObject {
            position: owner.position,
            id: target.id,
        }) {
            self.broke(BreakReason::Cycle { id: target.id });
            return Ok(());
        }

        let source = match self.path_sources {
            PathSourceMode::Omit => None,
            PathSourceMode::Retain => Some(HircPathSource::of(
                self.banks.archive(),
                &owner,
                object,
                self.inbound_source(target, owner.position),
            )),
        };
        let cursor = WalkCursor::new(
            object,
            owner.bank.header().version,
            self.banks.index,
            self.banks.choice,
            work,
        )?;
        let playback_features = self
            .playback_features()
            .combine(PlaybackFeatures::observe(&object.value));
        let step = PathStep {
            kind: node_kind(object),
            id: object.id,
        };
        // Bank objects are in physical order, including distinct occurrences of duplicate IDs.
        let start = object.byte_range().start;
        let object_number = owner
            .bank
            .objects()
            .partition_point(|candidate| candidate.byte_range().start < start);
        self.path.push(Frame {
            owner,
            object_number,
            step,
            source,
            playback_features,
            cursor,
        });
        Ok(())
    }

    fn current(&self) -> &Loaded<'i> {
        self.path
            .last()
            .map_or(&self.banks.home, |frame| &frame.owner)
    }

    fn inbound_source(&self, target: Target, position: u32) -> Option<HircInboundSource> {
        let parent = self.path.last()?;
        let bank_id = match target.via {
            Via::Bank(id) if position != parent.owner.position => Some(id),
            _ => None,
        };
        target.inbound?.source(
            self.banks.archive(),
            &parent.owner,
            parent.object()?,
            parent.source.as_ref(),
            bank_id,
        )
    }

    fn playback_features(&self) -> PlaybackFeatures {
        self.path
            .last()
            .map_or_default(|step| step.playback_features)
    }

    fn leave(&mut self) {
        if let Some(frame) = self.path.pop() {
            self.active.remove(&frame.active_object());
        }
        self.shared_path_prefix = self.shared_path_prefix.min(self.path.len());
    }

    fn sound(&mut self, source: SoundSource, music_clip_features: MusicClipFeatures) {
        let bank = self.current().soundbank;
        let located = locate(self.banks.index, bank, self.banks.choice, &source);
        if located.missing_localisation {
            self.broke(self.banks.choice.no_localisation());
        }
        self.found.variants.push(SoundVariant {
            event: self.event,
            soundbank: bank,
            sound_id: self.path.last().map_or(0, |frame| frame.step.id),
            path: self.public_path(),
            path_sources: self.path_sources(),
            shared_path_prefix: self.shared_path_prefix,
            music_clip_features,
            playback_features: self.playback_features(),
            source,
            location: located.location,
            selection: located.selection,
            localised: located.localised,
            language: located.language,
            fallback: located.fallback,
        });
        self.shared_path_prefix = self.path.len();
    }

    fn public_path(&self) -> Box<[PathStep]> {
        self.path.iter().map(|frame| frame.step).collect()
    }

    fn path_sources(&self) -> Box<[HircPathSource]> {
        match self.path_sources {
            PathSourceMode::Omit => Box::default(),
            PathSourceMode::Retain => self
                .path
                .iter()
                .filter_map(|frame| frame.source.clone())
                .collect(),
        }
    }

    fn broke(&mut self, reason: BreakReason) {
        self.broke_in(None, reason);
    }

    fn broke_in(&mut self, soundbank: Option<&'i IndexedSoundbank>, reason: BreakReason) {
        let soundbank = soundbank.unwrap_or_else(|| self.current().soundbank);
        self.found.breaks.push(BrokenLink {
            event: self.event,
            soundbank: Some(soundbank),
            path: self.public_path(),
            path_sources: self.path_sources(),
            reason,
        });
    }
}

const fn node_kind(object: &HircObject) -> NodeKind {
    match (&object.value, object.kind) {
        (HircValue::Container(container), _) => NodeKind::of_container(container.kind),
        (_, HircKind::Event) => NodeKind::Event,
        (_, HircKind::Action) => NodeKind::Action,
        (_, HircKind::Sound) => NodeKind::Sound,
        (_, kind) => NodeKind::Other(kind),
    }
}
