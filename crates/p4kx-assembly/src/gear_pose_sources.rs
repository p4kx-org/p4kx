// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::{EntryOrigin, SourceLocation};
use p4kx_model::{Mesh, Model};

/// The encoded channel of an evaluated landing gear key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum GearKeyChannel {
    /// An authored rotation.
    Rotation,
    /// An authored translation.
    Translation,
}

impl GearKeyChannel {
    /// Returns the channel name used by a source location.
    #[must_use]
    pub const fn field(self) -> &'static str {
        match self {
            Self::Rotation => "rotation",
            Self::Translation => "translation",
        }
    }
}

/// One value selected from a physically located clip and controller channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GearAnimationKey {
    pub(crate) controller: usize,
    pub(crate) ordinal: usize,
    pub(crate) channel: GearKeyChannel,
}

impl GearAnimationKey {
    /// Returns the original controller table ordinal.
    #[must_use]
    pub const fn controller(self) -> usize {
        self.controller
    }

    /// Returns the original key ordinal in the encoded channel.
    #[must_use]
    pub const fn ordinal(self) -> usize {
        self.ordinal
    }

    /// Returns the encoded channel.
    #[must_use]
    pub const fn channel(self) -> GearKeyChannel {
        self.channel
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct PosedBone {
    pub(crate) parent: Option<usize>,
    pub(crate) rotation: Option<GearAnimationKey>,
    pub(crate) translation: Option<GearAnimationKey>,
}

#[derive(Debug)]
pub(crate) struct PosedLod {
    pub(crate) ordinal: usize,
    pub(crate) bones: Vec<PosedBone>,
}

/// A deployed model paired with any retained original pose evidence.
#[derive(Debug)]
pub struct DeployedGear {
    pub(crate) model: Model,
    pub(crate) sources: Option<GearPoseSources>,
}

impl DeployedGear {
    pub(crate) const fn from_model(model: Model) -> Self {
        Self {
            model,
            sources: None,
        }
    }

    /// Returns the model with the deploy pose applied.
    #[must_use]
    pub fn into_model(self) -> Model {
        self.model
    }
}

/// The original sources retained for a successfully deployed gear model.
#[derive(Debug)]
pub struct GearPoseSources {
    pub(crate) clip: Option<EntryOrigin>,
    pub(crate) clip_ordinal: usize,
    pub(crate) selectors: Vec<(EntryOrigin, SourceLocation)>,
    pub(crate) lods: Vec<PosedLod>,
}

impl GearPoseSources {
    /// Returns the actual Archive Entry that supplied the deployed clip.
    #[must_use]
    pub const fn clip(&self) -> Option<&EntryOrigin> {
        self.clip.as_ref()
    }

    /// Returns the original ordinal of the deployed clip in that Entry.
    #[must_use]
    pub const fn clip_ordinal(&self) -> usize {
        self.clip_ordinal
    }

    /// Returns the original markup selectors consumed by this deploy route.
    pub fn selectors(&self) -> impl Iterator<Item = (&EntryOrigin, &SourceLocation)> {
        self.selectors
            .iter()
            .map(|(origin, location)| (origin, location))
    }
}

/// A deployed gear's bound model and its own pose evidence.
#[derive(Debug, Clone, Copy)]
pub struct GearPoseInput<'a> {
    pub(crate) model: &'a Model,
    pub(crate) source: &'a GearPoseSources,
}

impl<'a> GearPoseInput<'a> {
    /// Returns the physical source evidence of the deploy pose.
    #[must_use]
    pub const fn source(self) -> &'a GearPoseSources {
        self.source
    }

    /// Returns the deploy keys actually used by the selected mesh vertices.
    #[must_use]
    pub fn keys_for_vertices(
        self,
        lod_ordinal: usize,
        mesh: usize,
        vertices: impl Iterator<Item = usize>,
    ) -> BTreeSet<GearAnimationKey> {
        let Some(pose) = self
            .source
            .lods
            .iter()
            .find(|pose| pose.ordinal == lod_ordinal)
        else {
            return BTreeSet::new();
        };
        let Some(mesh) = self
            .model
            .lods
            .get(lod_ordinal)
            .and_then(|lod| lod.meshes.get(mesh))
        else {
            return BTreeSet::new();
        };
        pose.keys(mesh, vertices)
    }
}

impl PosedLod {
    fn keys(
        &self,
        mesh: &Mesh,
        vertices: impl Iterator<Item = usize>,
    ) -> BTreeSet<GearAnimationKey> {
        let Some(influences) = mesh
            .influences
            .as_ref()
            .filter(|data| data.len() == mesh.positions.len())
        else {
            return BTreeSet::new();
        };
        let mut visited = vec![false; self.bones.len()];
        let mut keys = BTreeSet::new();
        for influences in vertices.filter_map(|vertex| influences.get(vertex)) {
            for (&joint, &weight) in influences.joints.iter().zip(&influences.weights) {
                if weight > 0 {
                    self.keys_from_joint(usize::from(joint), &mut visited, &mut keys);
                }
            }
        }
        keys
    }

    fn keys_from_joint(
        &self,
        joint: usize,
        visited: &mut [bool],
        keys: &mut BTreeSet<GearAnimationKey>,
    ) {
        let mut index = Some(joint);
        while let Some((bone, visited)) =
            index.and_then(|at| self.bones.get(at).zip(visited.get_mut(at)))
        {
            if std::mem::replace(visited, true) {
                break;
            }
            keys.extend([bone.rotation, bone.translation].into_iter().flatten());
            index = bone.parent;
        }
    }
}
