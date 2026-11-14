// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::bank::BankSharedNode;

use super::{Common, Coverage, Input, Owner, TestResult, Visitor, compare, modulation};

pub(in super::super) fn music(
    visitor: &mut Visitor<'_>,
    owner: Owner,
    node: &mut BankSharedNode<'_, Input<'_>>,
) -> TestResult {
    visitor.mark(Coverage::Music);
    let parameters = node.parameters();
    visitor.common(
        owner,
        Common {
            parent: node.parent(),
            properties: &parameters.properties,
            aux: &parameters.aux,
            playback: &parameters.playback,
        },
    )?;
    modulation::state(visitor, owner, node.states())?;
    modulation::rtpc(visitor, owner, "music-node", node.rtpc())?;
    if let Some(output) = &visitor.output {
        verify_node_rows!(output, owner, node)?;
    }
    Ok(())
}
