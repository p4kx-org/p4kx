// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::Bounds;

/// A mesh whose vertices used a model node transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshNodeInput {
    pub(super) level: u32,
    pub(super) mesh: usize,
    pub(super) node: usize,
}

impl MeshNodeInput {
    /// Returns the actual level of detail.
    #[must_use]
    pub const fn level(self) -> u32 {
        self.level
    }

    /// Returns the decoded mesh ordinal within the level.
    #[must_use]
    pub const fn mesh(self) -> usize {
        self.mesh
    }

    /// Returns the decoded node ordinal used to transform the mesh.
    #[must_use]
    pub const fn node(self) -> usize {
        self.node
    }
}

#[derive(Clone, Copy)]
pub(super) enum MeshNodeCapture {
    Disabled,
    Enabled,
}

#[derive(Debug, Default)]
pub(super) struct MeshNodeTrace {
    pub(super) ranges: Vec<Option<MeshNodeInput>>,
    bounds: [Option<MeshNodeInput>; 6],
}

impl MeshNodeTrace {
    pub(super) fn update_bounds(
        &mut self,
        before: Bounds,
        after: Bounds,
        input: Option<MeshNodeInput>,
    ) {
        let previous = before.min.into_iter().chain(before.max);
        let current = after.min.into_iter().chain(after.max);
        for ((kept, previous), current) in self.bounds.iter_mut().zip(previous).zip(current) {
            if previous.to_bits() != current.to_bits() {
                *kept = input;
            }
        }
    }

    pub(super) fn into_bound_inputs(self) -> Vec<MeshNodeInput> {
        let mut inputs = Vec::new();
        for input in self.bounds.into_iter().flatten() {
            if !inputs.contains(&input) {
                inputs.push(input);
            }
        }
        inputs
    }
}

#[derive(Debug, Default)]
pub(super) struct MeshNodeInputs {
    pub(super) bounds: Vec<Vec<MeshNodeInput>>,
    pub(super) groups: Vec<Option<MeshNodeInput>>,
}
