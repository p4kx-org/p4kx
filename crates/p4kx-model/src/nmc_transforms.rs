// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::TryReserveError;

use crate::{NmcFrames, Transform};

/// Why a physical model node cannot establish a usable frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NmcFrameProblem {
    /// No row has the requested original ordinal.
    MissingNode,
    /// A parent ordinal falls outside the original table.
    InvalidParent,
    /// The parent chain contains a cycle.
    Cycle,
    /// A stored input or composed output is not finite.
    NonFinite,
    /// The local affine collapses a dimension.
    Singular,
}

/// One original node's local and composed frames.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct NmcNodeTransforms {
    /// The node's own authored transform with its separate scale applied.
    pub local: Result<Transform, NmcFrameProblem>,
    /// The composed transform from the node to model space.
    pub model: Result<Transform, NmcFrameProblem>,
}

/// A failure to construct the original node frame projection.
#[derive(Debug)]
#[non_exhaustive]
pub enum NmcTransformError<E> {
    /// The memory required by the original table could not be reserved.
    Allocation(TryReserveError),
    /// A caller checkpoint stopped processing.
    Checkpoint(E),
    /// An internal lineage row could not be recovered from the admitted table.
    InvalidState,
}

#[derive(Debug, Clone, Copy)]
enum FrameEntry {
    Unseen,
    Visiting,
    Ready(Transform),
    Invalid(NmcFrameProblem),
}

/// Projected affines for all original rows, including nodes without render geometry.
#[derive(Debug)]
pub struct NmcTransforms<'a> {
    nodes: NmcFrames<'a>,
    world: Vec<FrameEntry>,
}

impl<'a> NmcTransforms<'a> {
    /// Composes each original row once, polling between nodes and lineage steps.
    ///
    /// # Errors
    /// Returns memory allocation or caller checkpoint errors.
    pub fn read<E>(
        nodes: NmcFrames<'a>,
        mut check: impl FnMut() -> Result<(), E>,
    ) -> Result<Self, NmcTransformError<E>> {
        let mut world = Vec::new();
        let mut route = Vec::new();
        world
            .try_reserve_exact(nodes.len())
            .map_err(NmcTransformError::Allocation)?;

        route
            .try_reserve_exact(nodes.len())
            .map_err(NmcTransformError::Allocation)?;
        world.resize(nodes.len(), FrameEntry::Unseen);
        for index in 0..nodes.len() {
            let anchor = trace(nodes, &mut world, &mut route, index, &mut check)?;
            resolve(nodes, &mut world, &mut route, anchor, &mut check)?;
        }
        Ok(Self { nodes, world })
    }

    /// Returns the explicit outcome at one original ordinal.
    #[must_use]
    pub fn frame(&self, index: usize) -> NmcNodeTransforms {
        let local = self
            .nodes
            .frame(index)
            .ok_or(NmcFrameProblem::MissingNode)
            .and_then(local_transform);
        let model = match self.world.get(index) {
            Some(FrameEntry::Ready(world)) => Ok(*world),
            Some(FrameEntry::Invalid(problem)) => Err(*problem),
            _ => Err(NmcFrameProblem::MissingNode),
        };
        NmcNodeTransforms { local, model }
    }
}

fn trace<E>(
    nodes: NmcFrames<'_>,
    world: &mut [FrameEntry],
    route: &mut Vec<usize>,
    start: usize,
    check: &mut impl FnMut() -> Result<(), E>,
) -> Result<FrameEntry, NmcTransformError<E>> {
    let mut index = start;
    loop {
        check().map_err(NmcTransformError::Checkpoint)?;
        let Some((entry, row)) = world.get_mut(index).zip(nodes.frame(index)) else {
            return Ok(FrameEntry::Invalid(NmcFrameProblem::InvalidParent));
        };
        match *entry {
            FrameEntry::Unseen => {
                *entry = FrameEntry::Visiting;
                route.push(index);
            }
            FrameEntry::Visiting => return Ok(FrameEntry::Invalid(NmcFrameProblem::Cycle)),
            entry => return Ok(entry),
        }
        let parent = row.parent_word();
        if parent == u16::MAX {
            return Ok(FrameEntry::Ready(Transform::IDENTITY));
        }
        index = usize::from(parent);
    }
}

fn resolve<E>(
    nodes: NmcFrames<'_>,
    world: &mut [FrameEntry],
    route: &mut Vec<usize>,
    mut anchor: FrameEntry,
    check: &mut impl FnMut() -> Result<(), E>,
) -> Result<(), NmcTransformError<E>> {
    while let Some(index) = route.pop() {
        check().map_err(NmcTransformError::Checkpoint)?;
        let Some((entry, row)) = world.get_mut(index).zip(nodes.frame(index)) else {
            return Err(NmcTransformError::InvalidState);
        };
        if let FrameEntry::Ready(parent) = anchor {
            anchor = match local_transform(row) {
                Ok(local) => parent.then(&local).finite().map_or(
                    FrameEntry::Invalid(NmcFrameProblem::NonFinite),
                    FrameEntry::Ready,
                ),
                Err(problem) => FrameEntry::Invalid(problem),
            };
        }
        *entry = anchor;
    }
    Ok(())
}

fn local_transform(row: crate::NmcFrame<'_>) -> Result<Transform, NmcFrameProblem> {
    let matrix = row.local_matrix_bits().map(|line| line.map(f32::from_bits));
    let scale = row.scale_bits().map(f32::from_bits);
    if matrix
        .iter()
        .flatten()
        .chain(&scale)
        .any(|value| !value.is_finite())
    {
        return Err(NmcFrameProblem::NonFinite);
    }
    let local = if matrix.iter().flatten().all(|value| *value == 0.0) {
        Transform::IDENTITY
    } else {
        Transform { rows: matrix }
    };
    let scaled =
        if scale.iter().all(|value| *value == 0.0) || row.scale_bits() == [1.0_f32.to_bits(); 3] {
            local
        } else {
            if scale.contains(&0.0) {
                return Err(NmcFrameProblem::Singular);
            }
            local
                .scaled(scale.map(f64::from))
                .ok_or(NmcFrameProblem::NonFinite)?
        };
    scaled.inverse().ok_or(NmcFrameProblem::Singular)?;
    Ok(scaled)
}
