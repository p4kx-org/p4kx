// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use crate::PlayerSourceCapture;
use crate::player_bundle_wire::Row;

pub(super) fn write(
    capture: &PlayerSourceCapture,
    mut push: impl FnMut(&Row<'_>) -> io::Result<()>,
) -> io::Result<()> {
    for (index, body) in capture.bodies.iter().enumerate() {
        push(&Row::Body {
            index,
            body_variant: body.body_variant,
            value: &body.body,
        })?;
    }
    for (index, value) in capture.loadouts.iter().enumerate() {
        push(&Row::Loadout { index, value })?;
    }
    push(&Row::MotionActor {
        value: &capture.motion.actor,
    })?;
    let components = &capture.motion.components;
    push(&Row::MotionComponents {
        state: components.state(),
        origin: components.origin(),
    })?;
    for (index, value) in components.value().into_iter().flatten().enumerate() {
        value.bundle_rows(index, &mut push)?;
    }
    for adb in &capture.adb_declarations {
        push(&Row::AdbDocument {
            document: adb.document,
            source: &adb.source,
        })?;
        for (index, value) in adb.fragments.iter().enumerate() {
            push(&Row::AdbFragment {
                document: adb.document,
                index,
                value,
            })?;
        }
    }
    for (index, value) in capture.adb_tag_uses.scopes.iter().enumerate() {
        push(&Row::AdbTagScope { index, value })?;
    }
    for value in &capture.adb_tag_uses.selectors {
        push(&Row::AdbTagSelectors { value })?;
    }
    for (index, value) in capture.tag_definitions.roots.iter().enumerate() {
        push(&Row::TagDefinitionRoot { index, value })?;
    }
    for value in &capture.tag_definitions.documents {
        push(&Row::TagDefinitionDocument { value })?;
    }
    for value in &capture.tag_definitions.cycle_back_edges {
        push(&Row::TagCycleBackEdge { value })?;
    }
    for value in capture.source_inventory.sources() {
        push(&Row::Source { value })?;
    }
    for value in capture.source_inventory.uses() {
        push(&Row::SourceUse { value })?;
    }
    Ok(())
}
