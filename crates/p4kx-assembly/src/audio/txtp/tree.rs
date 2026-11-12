// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::{HircKind, MusicTransitionFeatures};

use crate::audio::music_clip_features::MusicClipFeatures;
use crate::audio::{NodeKind, PathStep, SoundVariant, SourceKind};

/// How the children of a group play together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Combine {
    /// All at once: an Event's actions, a blend container, a music segment or track.
    Layers,
    /// One after another: a sequence container or a music playlist.
    Segments,
    /// One of them, chosen at run time: a random or switch container or a music switch.
    Select,
}

impl Combine {
    const fn of(kind: NodeKind) -> Self {
        match kind {
            NodeKind::Sequence | NodeKind::Other(HircKind::MusicPlaylist) => Self::Segments,
            NodeKind::Random
            | NodeKind::Switch
            | NodeKind::Other(HircKind::MusicSwitch | HircKind::DialogueEvent) => Self::Select,
            _ => Self::Layers,
        }
    }
}

/// A sound a group plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Leaf {
    /// The WEM ID, `None` for a source that has no file.
    pub(super) wem_id: Option<u32>,
    /// The media ID the Sound object states.
    pub(super) source_id: u32,
    pub(super) kind: SourceKind,
    pub(super) music_clip_features: MusicClipFeatures,
    pub(super) music_transition_features: MusicTransitionFeatures,
}

/// A child of a group: a sound or another group.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Node {
    Leaf(Leaf),
    Group(Group),
}

/// An object of the path that plays several children.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Group {
    pub(super) step: PathStep,
    pub(super) combine: Combine,
    pub(super) children: Vec<Node>,
}

impl Group {
    const fn new(step: PathStep) -> Self {
        Self {
            step,
            combine: Combine::of(step.kind),
            children: Vec::new(),
        }
    }

    /// Adds a leaf in visit order, reusing only the groups still open in the source walk.
    fn insert(&mut self, steps: &[PathStep], shared_prefix: usize, leaf: Leaf) {
        let Some((step, rest)) = steps.split_first() else {
            self.children.push(Node::Leaf(leaf));
            return;
        };
        if shared_prefix > 0
            && let Some(Node::Group(last)) = self.children.last_mut()
            && last.step == *step
        {
            last.insert(rest, shared_prefix - 1, leaf);
            return;
        }
        let mut group = Self::new(*step);
        group.insert(rest, 0, leaf);
        self.children.push(Node::Group(group));
    }
}

/// Rebuilds the tree of objects the variants of one Wwise event play, from the Event object down.
/// The paths are at most `MAX_PATH_DEPTH` long, so the tree is as deep.
///
/// A Sound object is a leaf; the last object of a music variant's path, a music track, holds its
/// sources as leaves. Returns `None` for no variants.
pub(super) fn build(variants: &[SoundVariant<'_>]) -> Option<Group> {
    let root = *variants.first()?.path().first()?;
    let mut holders = variants
        .chunk_by(|_, next| next.shared_path_prefix > 0)
        .map(|holder| build_holder(root, holder));
    let first = holders.next()?;
    let Some(second) = holders.next() else {
        return Some(first);
    };
    let mut tree = Group::new(root);
    tree.children
        .extend([first, second].into_iter().chain(holders).map(Node::Group));
    Some(tree)
}

fn build_holder(root: PathStep, variants: &[SoundVariant<'_>]) -> Group {
    let mut tree = Group::new(root);
    for variant in variants {
        let path = variant.path();
        let below = path.get(1..).unwrap_or_default();
        let owners = match below.split_last() {
            Some((last, owners)) if last.kind == NodeKind::Sound => owners,
            _ => below,
        };
        tree.insert(
            owners,
            variant.shared_path_prefix.saturating_sub(1),
            Leaf {
                wem_id: variant.wem_id(),
                source_id: variant.source().source_id,
                kind: variant.source_kind(),
                music_clip_features: variant.music_clip_features,
                music_transition_features: variant.music_transition_features,
            },
        );
    }
    tree
}
