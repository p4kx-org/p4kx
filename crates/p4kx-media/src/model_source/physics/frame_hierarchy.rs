// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::{NmcFrameProblem, NmcFrames, NmcTransformError, NmcTransforms};
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

impl From<NmcFrameProblem> for FrameDisposition {
    fn from(problem: NmcFrameProblem) -> Self {
        match problem {
            NmcFrameProblem::MissingNode => Self::MissingNode,
            NmcFrameProblem::InvalidParent => Self::InvalidParent,
            NmcFrameProblem::Cycle => Self::Cycle,
            NmcFrameProblem::NonFinite => Self::NonFinite,
            NmcFrameProblem::Singular => Self::Singular,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct PhysicsFrame {
    pub(super) disposition: FrameDisposition,
    pub(super) node_chunk: Option<usize>,
    pub(super) local: Option<[[f32; 4]; 3]>,
    pub(super) entity: Option<[[f32; 4]; 3]>,
}

pub(super) fn read<'a>(
    nodes: NmcFrames<'a>,
    writer: &crate::MediaWriter<'_>,
) -> Result<NmcTransforms<'a>, crate::MediaError> {
    NmcTransforms::read(nodes, || writer.checkpoint()).map_err(|error| match error {
        NmcTransformError::Allocation(cause) => crate::MediaError::Contract(cause.to_string()),
        NmcTransformError::Checkpoint(cause) => cause,
        _ => crate::MediaError::Contract("unsupported NMC projection failure".into()),
    })
}

pub(super) fn frame(hierarchy: &NmcTransforms<'_>, chunk: usize, index: usize) -> PhysicsFrame {
    let frame = hierarchy.frame(index);
    PhysicsFrame {
        disposition: frame
            .model
            .map_or_else(FrameDisposition::from, |_| FrameDisposition::Resolved),
        node_chunk: Some(chunk),
        local: frame.local.ok().map(|value| value.rows),
        entity: frame.model.ok().map(|value| value.rows),
    }
}
