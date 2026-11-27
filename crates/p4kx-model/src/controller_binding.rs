// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use crate::Bone;

/// The selection and duplicate facts for one authored controller occurrence.
#[derive(Debug, Clone, Copy)]
pub struct ControllerBinding {
    first_controller: usize,
    bone: Option<BoneMatches>,
    bound: Option<usize>,
    first_eligible: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
struct BoneMatches {
    first: usize,
    count: usize,
}

impl ControllerBinding {
    /// Returns the first authored controller ordinal with the same ID.
    #[must_use]
    pub const fn first_controller(self) -> usize {
        self.first_controller
    }

    /// Returns the first original matching bone ordinal, even for a duplicate controller.
    #[must_use]
    pub const fn first_bone(self) -> Option<usize> {
        match self.bone {
            Some(bone) => Some(bone.first),
            None => None,
        }
    }

    /// Returns how many authored bones share this ID.
    #[must_use]
    pub const fn matching_bones(self) -> usize {
        match self.bone {
            Some(bone) => bone.count,
            None => 0,
        }
    }

    /// Returns the first eligible controller with this ID, when one was observed.
    #[must_use]
    pub const fn first_eligible(self) -> Option<usize> {
        self.first_eligible
    }

    /// Returns the bound bone of the first eligible controller that names it.
    #[must_use]
    pub const fn bound_bone(self) -> Option<usize> {
        self.bound
    }
}

#[derive(Debug)]
pub(crate) struct BoneControllers(HashMap<u32, BoneMatches>);

impl BoneControllers {
    pub(crate) fn new(bones: &[Bone]) -> Self {
        let mut by_id: HashMap<u32, BoneMatches> = HashMap::with_capacity(bones.len());
        for (index, bone) in bones.iter().enumerate() {
            by_id
                .entry(bone.controller_id)
                .and_modify(|matches| matches.count += 1)
                .or_insert(BoneMatches {
                    first: index,
                    count: 1,
                });
        }
        Self(by_id)
    }

    pub(crate) fn binder(&self) -> ControllerBinder<'_> {
        ControllerBinder {
            bones: self,
            controllers: HashMap::new(),
        }
    }
}

/// Binds the authored controller occurrences of one clip in original order.
#[derive(Debug)]
pub struct ControllerBinder<'skeleton> {
    bones: &'skeleton BoneControllers,
    controllers: HashMap<u32, ControllerOrder>,
}

/// Whether a source controller can occupy a bone binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ControllerEligibility {
    /// The controller has a key-bearing track.
    Track,
    /// The controller is authored without a track.
    Empty,
}

#[derive(Debug)]
struct ControllerOrder {
    first: usize,
    first_eligible: Option<usize>,
}

impl ControllerBinder<'_> {
    /// Observes the next original occurrence with its physical controller ordinal.
    #[must_use]
    pub fn observe(
        &mut self,
        ordinal: usize,
        id: u32,
        eligibility: ControllerEligibility,
    ) -> ControllerBinding {
        let order = self.controllers.entry(id).or_insert(ControllerOrder {
            first: ordinal,
            first_eligible: None,
        });
        let bone = self.bones.0.get(&id).copied();
        let bound = if eligibility == ControllerEligibility::Track && order.first_eligible.is_none()
        {
            order.first_eligible = Some(ordinal);
            bone.map(|matches| matches.first)
        } else {
            None
        };
        ControllerBinding {
            first_controller: order.first,
            bone,
            bound,
            first_eligible: order.first_eligible,
        }
    }
}
