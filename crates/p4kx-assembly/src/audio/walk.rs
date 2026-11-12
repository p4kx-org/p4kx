// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::rc::Rc;

use p4kx_audio::{HircKind, HircObject, HircValue, SoundSource, Soundbank};

use super::locate::locate;
use super::walk_banks::{Banks, Loaded, Via, select_holders};
use super::{
    AudioIndex, BreakReason, BrokenLink, IndexedSoundbank, LanguageChoice, MAX_ACTION_CHAIN,
    MAX_CONTAINER_CHILDREN, MAX_PATH_DEPTH, NodeKind, PathStep, SoundVariant, WwiseEvent,
};

/// The variants and broken links one Wwise event leads to in one soundbank.
#[derive(Debug, Default)]
pub(crate) struct Found<'i> {
    pub(crate) variants: Vec<SoundVariant<'i>>,
    pub(crate) breaks: Vec<BrokenLink<'i>>,
}

/// Follows the Wwise event `event` through the `HIRC` section of the start soundbank of `banks`
/// to its sounds through indexed bank references.
pub(crate) fn event<'i>(banks: Banks<'_, 'i>, event: &'i WwiseEvent, budget: usize) -> Found<'i> {
    let mut walk = Walk {
        index: banks.index,
        choice: banks.choice,
        current: banks.home.clone(),
        banks,
        event,
        path: Vec::new(),
        shared_path_prefix: 0,
        objects: 0,
        budget,
        chain: 0,
        exhausted: false,
        found: Found::default(),
    };
    walk.follow(event.id());
    walk.found
}

struct ResolvedPathStep<'index> {
    owner: Loaded<'index>,
    step: PathStep,
}

struct Walk<'w, 'i> {
    index: &'i AudioIndex,
    choice: &'w LanguageChoice,
    banks: Banks<'w, 'i>,
    current: Loaded<'i>,
    event: &'i WwiseEvent,
    path: Vec<ResolvedPathStep<'i>>,
    shared_path_prefix: usize,
    objects: usize,
    budget: usize,
    chain: usize,
    exhausted: bool,
    found: Found<'i>,
}

impl<'i> Walk<'_, 'i> {
    fn follow(&mut self, id: u32) {
        self.reach(id, None, Via::Local);
    }

    fn spend(&mut self) -> bool {
        if self.objects >= self.budget {
            if !self.exhausted {
                self.exhausted = true;
                self.broke(BreakReason::TooManyObjects { limit: self.budget });
            }
            return false;
        }
        self.objects += 1;
        true
    }

    fn reach(&mut self, id: u32, wanted: Option<HircKind>, via: Via) {
        if !self.spend() {
            return;
        }
        if self.path.len() >= MAX_PATH_DEPTH {
            self.broke(BreakReason::TooDeep {
                limit: MAX_PATH_DEPTH,
            });
            return;
        }
        let wanted = wanted.or_else(|| self.path.is_empty().then_some(HircKind::Event));
        let in_current = match via {
            Via::At(position) => position == self.current.position,
            Via::Local | Via::Bank(_) | Via::PlayEvent => true,
        };
        let (bank, entered) = if in_current && find(&self.current.bank, id, wanted).is_some() {
            (Rc::clone(&self.current.bank), None)
        } else {
            match self.banks.enter(
                self.current.position,
                id,
                via,
                self.path.iter().map(|active| &active.owner),
            ) {
                Ok(next) => (Rc::clone(&next.bank), Some(next)),
                Err(refused) => {
                    self.broke_in(refused.bank, refused.reason);
                    return;
                }
            }
        };
        let Some(object) = find(&bank, id, wanted) else {
            self.broke(BreakReason::Missing { id });
            return;
        };
        let position = entered
            .as_ref()
            .map_or(self.current.position, |next| next.position);
        #[cfg(test)]
        self.banks.probe_selection(position, object.id, &bank);
        if self
            .path
            .iter()
            .any(|active| active.owner.position == position && active.step.id == id)
        {
            self.broke(BreakReason::Cycle { id });
            return;
        }
        let left = entered.map(|next| std::mem::replace(&mut self.current, next));
        self.path.push(ResolvedPathStep {
            owner: self.current.clone(),
            step: PathStep {
                kind: node_kind(object),
                id,
            },
        });
        self.visit(object);
        self.leave(left);
    }

    fn leave(&mut self, previous: Option<Loaded<'i>>) {
        self.path.pop();
        self.shared_path_prefix = self.shared_path_prefix.min(self.path.len());
        if let Some(previous) = previous {
            self.current = previous;
        }
    }

    fn visit(&mut self, object: &HircObject) {
        match &object.value {
            HircValue::Event(actions) => self.children(actions),
            HircValue::Action(action) => {
                if action.is_play() {
                    let via = action
                        .bank
                        .map_or(Via::Local, |bank| Via::Bank(bank.bank_id));
                    self.reach(action.target_id, None, via);
                } else if action.is_play_event() {
                    self.chained(|walk| {
                        walk.reach(action.target_id, Some(HircKind::Event), Via::PlayEvent);
                    });
                } else if action.is_trigger() {
                    self.chained(|walk| walk.stingers(action.target_id));
                }
            }
            HircValue::Container(container) => self.children(&container.children),
            HircValue::MusicSegment(segment) => self.children(&segment.node.children),
            HircValue::MusicSwitch(switch) => self.children(&switch.node.children),
            HircValue::MusicPlaylist(playlist) => self.children(&playlist.node.children),
            HircValue::MusicTrack(track) => {
                for source in track.sources.iter().take(MAX_CONTAINER_CHILDREN) {
                    if !self.spend() {
                        break;
                    }
                    self.sound(*source);
                }
                if track.sources.len() > MAX_CONTAINER_CHILDREN {
                    self.broke(BreakReason::TooManyChildren {
                        count: track.sources.len(),
                        limit: MAX_CONTAINER_CHILDREN,
                    });
                }
            }
            HircValue::Sound(source) => self.sound(*source),
            HircValue::ActorMixer(_)
            | HircValue::State(_)
            | HircValue::Malformed(_)
            | HircValue::Raw => self.unresolved_object(object),
        }
    }

    fn unresolved_object(&mut self, object: &HircObject) {
        let reason = match &object.value {
            HircValue::Malformed(error)
                if !matches!(object.kind, HircKind::State | HircKind::ActorMixer) =>
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
                BreakReason::UnknownLayout {
                    version: self.current.bank.header().version,
                }
            }
            _ => BreakReason::Unsupported,
        };
        self.broke(reason);
    }

    fn chained(&mut self, follow: impl FnOnce(&mut Self)) {
        if self.chain >= MAX_ACTION_CHAIN {
            self.broke(BreakReason::ChainTooLong {
                limit: MAX_ACTION_CHAIN,
            });
            return;
        }
        self.chain += 1;
        follow(self);
        self.chain -= 1;
    }

    fn stingers(&mut self, trigger: u32) {
        let stingers = self.index.stingers(trigger);
        if stingers.is_empty() {
            self.broke(BreakReason::Missing { id: trigger });
            return;
        }
        let mut banks: Vec<u32> = stingers.iter().map(|stinger| stinger.bank).collect();
        banks.sort_unstable();
        banks.dedup();
        let chosen = select_holders(self.index, &banks, self.choice);
        if chosen.is_empty() {
            self.broke(self.choice.no_localisation());
            return;
        }
        let wanted: Vec<_> = stingers
            .iter()
            .filter(|stinger| chosen.contains(&stinger.bank))
            .collect();
        for stinger in wanted.iter().take(MAX_CONTAINER_CHILDREN) {
            self.reach(stinger.segment_id, None, Via::At(stinger.bank));
        }
        if wanted.len() > MAX_CONTAINER_CHILDREN {
            self.broke(BreakReason::TooManyChildren {
                count: wanted.len(),
                limit: MAX_CONTAINER_CHILDREN,
            });
        }
    }

    fn children(&mut self, ids: &[u32]) {
        for &id in ids.iter().take(MAX_CONTAINER_CHILDREN) {
            self.follow(id);
        }
        if ids.len() > MAX_CONTAINER_CHILDREN {
            self.broke(BreakReason::TooManyChildren {
                count: ids.len(),
                limit: MAX_CONTAINER_CHILDREN,
            });
        }
    }

    fn sound(&mut self, source: SoundSource) {
        let located = locate(self.index, self.current.soundbank, self.choice, &source);
        if located.missing_localisation {
            self.broke(self.choice.no_localisation());
        }
        self.found.variants.push(SoundVariant {
            event: self.event,
            soundbank: self.current.soundbank,
            sound_id: self.path.last().map_or(0, |step| step.step.id),
            path: self.public_path(),
            shared_path_prefix: self.shared_path_prefix,
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
        self.path.iter().map(|active| active.step).collect()
    }

    fn broke(&mut self, reason: BreakReason) {
        self.broke_in(None, reason);
    }

    fn broke_in(&mut self, soundbank: Option<&'i IndexedSoundbank>, reason: BreakReason) {
        self.found.breaks.push(BrokenLink {
            event: self.event,
            soundbank: Some(soundbank.unwrap_or(self.current.soundbank)),
            path: self.public_path(),
            reason,
        });
    }
}

fn find(bank: &Soundbank<Vec<u8>>, id: u32, wanted: Option<HircKind>) -> Option<&HircObject> {
    let found = bank.object(id)?;
    match wanted {
        Some(kind) if found.kind != kind => bank
            .objects()
            .iter()
            .find(|object| object.kind == kind && object.id == id),
        _ => Some(found),
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
