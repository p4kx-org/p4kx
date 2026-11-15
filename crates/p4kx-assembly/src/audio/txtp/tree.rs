// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::HircKind;

use crate::audio::music_clip_features::MusicClipFeatures;
use crate::audio::playback_features::PlaybackFeatures;
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
    pub(super) playback_features: PlaybackFeatures,
    pub(super) variant: usize,
}

/// A child of a group: a sound or another group.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Node {
    Leaf(Leaf),
    Group(GroupId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct GroupId(usize);

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
}

/// A playback tree whose group links are indices, independent of native stack depth.
pub(super) struct Tree {
    groups: Vec<Group>,
}

impl Tree {
    pub(super) const ROOT: GroupId = GroupId(0);

    pub(super) fn new(root: PathStep) -> Self {
        Self {
            groups: vec![Group::new(root)],
        }
    }

    pub(super) fn group(&self, id: GroupId) -> Option<&Group> {
        self.groups.get(id.0)
    }

    pub(super) fn add_group(&mut self, step: PathStep) -> GroupId {
        let id = GroupId(self.groups.len());
        self.groups.push(Group::new(step));
        id
    }

    pub(super) fn add_child(&mut self, parent: GroupId, child: Node) -> Option<()> {
        self.groups.get_mut(parent.0)?.children.push(child);
        Some(())
    }

    /// Adds a leaf in visit order, reusing only the groups still open in the source walk.
    pub(super) fn insert(
        &mut self,
        mut parent: GroupId,
        steps: &[PathStep],
        mut shared_prefix: usize,
        leaf: Leaf,
    ) -> Option<()> {
        for &step in steps {
            if shared_prefix > 0
                && let Some(existing) = self.last_group(parent, step)
            {
                parent = existing;
                shared_prefix -= 1;
                continue;
            }
            let id = self.add_group(step);
            self.add_child(parent, Node::Group(id))?;
            parent = id;
            shared_prefix = 0;
        }
        self.add_child(parent, Node::Leaf(leaf))
    }

    fn last_group(&self, parent: GroupId, step: PathStep) -> Option<GroupId> {
        let Node::Group(child) = self.group(parent)?.children.last()? else {
            return None;
        };
        (self.group(*child)?.step == step).then_some(*child)
    }
}

/// Rebuilds the objects played by the variants of one Wwise event, from the Event down.
///
/// A Sound is a leaf. The final `MusicTrack` of a music variant holds its sources as leaves.
pub(super) fn build(variants: &[SoundVariant<'_>]) -> Option<Tree> {
    let root = *variants.first()?.path().first()?;
    let mut holders = variants.chunk_by(|_, next| next.shared_path_prefix > 0);
    let first = holders.next()?;
    let mut tree = Tree::new(root);
    let Some(second) = holders.next() else {
        build_holder(&mut tree, Tree::ROOT, first, 0)?;
        return Some(tree);
    };
    let mut first_variant = 0;
    for holder in [first, second].into_iter().chain(holders) {
        let group = tree.add_group(root);
        build_holder(&mut tree, group, holder, first_variant)?;
        tree.add_child(Tree::ROOT, Node::Group(group))?;
        first_variant += holder.len();
    }
    Some(tree)
}

fn build_holder(
    tree: &mut Tree,
    root: GroupId,
    variants: &[SoundVariant<'_>],
    first_variant: usize,
) -> Option<()> {
    for (offset, variant) in variants.iter().enumerate() {
        let below = variant.path().get(1..).unwrap_or_default();
        let owners = match below.split_last() {
            Some((last, owners)) if last.kind == NodeKind::Sound => owners,
            _ => below,
        };
        tree.insert(
            root,
            owners,
            variant.shared_path_prefix.saturating_sub(1),
            Leaf {
                wem_id: variant.wem_id(),
                source_id: variant.source().source_id,
                kind: variant.source_kind(),
                music_clip_features: variant.music_clip_features,
                playback_features: variant.playback_features,
                variant: first_variant + offset,
            },
        )?;
    }
    Some(())
}
