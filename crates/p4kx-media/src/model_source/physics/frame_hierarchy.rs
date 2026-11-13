// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::{NmcFrames, Transform};
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum FrameDisposition {
    Resolved,
    WholeObject,
    MissingNodeTable,
    AmbiguousNodeTable,
    UnsupportedNodeTable,
    MalformedNodeTable,
    MissingNode,
    InvalidParent,
    Cycle,
    NonFinite,
    Singular,
}

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct PhysicsFrame {
    pub(super) disposition: FrameDisposition,
    pub(super) node_chunk: Option<usize>,
    pub(super) local: Option<[[f32; 4]; 3]>,
    pub(super) entity: Option<[[f32; 4]; 3]>,
}

#[derive(Debug, Clone, Copy)]
enum FrameEntry {
    Unseen,
    Visiting,
    Ready(Transform),
    Invalid(FrameDisposition),
}

pub(super) struct FrameHierarchy {
    world: Vec<FrameEntry>,
}

impl FrameHierarchy {
    pub(super) fn new(
        nodes: NmcFrames<'_>,
        writer: &crate::MediaWriter<'_>,
    ) -> Result<Self, crate::MediaError> {
        let mut world = Vec::new();
        let mut route = Vec::new();
        world
            .try_reserve_exact(nodes.len())
            .map_err(|error| allocation_error(&error))?;
        route
            .try_reserve_exact(nodes.len())
            .map_err(|error| allocation_error(&error))?;
        world.resize(nodes.len(), FrameEntry::Unseen);
        for index in 0..nodes.len() {
            writer.checkpoint()?;
            let anchor = trace(nodes, &mut world, &mut route, index, writer)?;
            resolve(nodes, &mut world, &mut route, anchor, writer)?;
        }
        Ok(Self { world })
    }

    pub(super) fn frame(&self, nodes: NmcFrames<'_>, chunk: usize, index: usize) -> PhysicsFrame {
        let local = nodes
            .frame(index)
            .and_then(|row| local_transform(row).ok())
            .map(|value| value.rows);
        let (disposition, entity) = match self.world.get(index) {
            Some(FrameEntry::Ready(world)) => (FrameDisposition::Resolved, Some(world.rows)),
            Some(FrameEntry::Invalid(problem)) => (*problem, None),
            _ => (FrameDisposition::MissingNode, None),
        };
        PhysicsFrame {
            disposition,
            node_chunk: Some(chunk),
            local,
            entity,
        }
    }
}

fn trace(
    nodes: NmcFrames<'_>,
    world: &mut [FrameEntry],
    route: &mut Vec<usize>,
    start: usize,
    writer: &crate::MediaWriter<'_>,
) -> Result<FrameEntry, crate::MediaError> {
    let mut index = start;
    loop {
        writer.checkpoint()?;
        let Some(entry) = world.get_mut(index) else {
            return Ok(FrameEntry::Invalid(FrameDisposition::InvalidParent));
        };
        match *entry {
            FrameEntry::Unseen => {
                *entry = FrameEntry::Visiting;
                route.push(index);
            }
            FrameEntry::Visiting => return Ok(FrameEntry::Invalid(FrameDisposition::Cycle)),
            entry => return Ok(entry),
        }
        let parent = nodes.frame(index).ok_or_else(internal_error)?.parent_word();
        if parent == u16::MAX {
            return Ok(FrameEntry::Ready(Transform::IDENTITY));
        }
        index = usize::from(parent);
    }
}

fn resolve(
    nodes: NmcFrames<'_>,
    world: &mut [FrameEntry],
    route: &mut Vec<usize>,
    mut anchor: FrameEntry,
    writer: &crate::MediaWriter<'_>,
) -> Result<(), crate::MediaError> {
    while let Some(index) = route.pop() {
        writer.checkpoint()?;
        if let FrameEntry::Ready(parent) = anchor {
            let row = nodes.frame(index).ok_or_else(internal_error)?;
            anchor = match local_transform(row) {
                Ok(local) => parent.then(&local).finite().map_or(
                    FrameEntry::Invalid(FrameDisposition::NonFinite),
                    FrameEntry::Ready,
                ),
                Err(problem) => FrameEntry::Invalid(problem),
            };
        }
        *world.get_mut(index).ok_or_else(internal_error)? = anchor;
    }
    Ok(())
}

fn local_transform(
    row: p4kx_extraction::model::NmcFrame<'_>,
) -> Result<Transform, FrameDisposition> {
    let matrix = row.local_matrix_bits().map(|line| line.map(f32::from_bits));
    let scale = row.scale_bits().map(f32::from_bits);
    if matrix
        .iter()
        .flatten()
        .chain(&scale)
        .any(|value| !value.is_finite())
    {
        return Err(FrameDisposition::NonFinite);
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
                return Err(FrameDisposition::Singular);
            }
            local
                .scaled(scale.map(f64::from))
                .ok_or(FrameDisposition::NonFinite)?
        };
    scaled.inverse().ok_or(FrameDisposition::Singular)?;
    Ok(scaled)
}

fn allocation_error(error: &std::collections::TryReserveError) -> crate::MediaError {
    crate::MediaError::Contract(error.to_string())
}

fn internal_error() -> crate::MediaError {
    crate::MediaError::Contract("NMC hierarchy lost an original row".into())
}
