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
    for value in &capture.controller_topology {
        push(&Row::ControllerTopology { value })?;
    }
    rig_rows(capture, &mut push)?;
    clip_rows(capture, &mut push)?;
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
    tag_rows(capture, &mut push)?;
    weapon_rows(capture, &mut push)?;
    for value in capture.source_inventory.sources() {
        push(&Row::Source { value })?;
    }
    for value in capture.source_inventory.uses() {
        push(&Row::SourceUse { value })?;
    }
    Ok(())
}

fn tag_rows(
    capture: &PlayerSourceCapture,
    push: &mut impl FnMut(&Row<'_>) -> io::Result<()>,
) -> io::Result<()> {
    for (index, value) in capture.adb_tag_uses.scopes.iter().enumerate() {
        push(&Row::AdbTagScope { index, value })?;
    }
    for value in &capture.adb_tag_uses.selectors {
        push(&Row::AdbTagSelectors { value })?;
    }
    for value in &capture.adb_tag_uses.branches {
        push(&Row::AdbBranchSelectors { value })?;
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
    Ok(())
}

fn rig_rows(
    capture: &PlayerSourceCapture,
    push: &mut impl FnMut(&Row<'_>) -> io::Result<()>,
) -> io::Result<()> {
    if let Some(candidates) = &capture.actor_rig_candidates {
        push(&Row::ActorRigCandidates {
            state: candidates.state(),
            origin: candidates.origin(),
        })?;
        for (index, value) in candidates.value().into_iter().flatten().enumerate() {
            push(&Row::ActorRigCandidate { index, value })?;
        }
    }
    Ok(())
}

fn clip_rows(
    capture: &PlayerSourceCapture,
    push: &mut impl FnMut(&Row<'_>) -> io::Result<()>,
) -> io::Result<()> {
    if let Some(value) = &capture.rig_clips.image {
        push(&Row::RigClipImage { value })?;
    }
    for (index, value) in capture.rig_clips.rigs.iter().enumerate() {
        push(&Row::RigClipRig { index, value })?;
    }
    for (rig, index, value) in &capture.rig_clips.rules {
        push(&Row::RigClipRule {
            rig: *rig,
            index: *index,
            value,
        })?;
    }
    for (rig, index, value) in &capture.rig_clips.banks {
        push(&Row::RigClipBank {
            rig: *rig,
            index: *index,
            value,
        })?;
    }
    for (rig, value) in &capture.rig_clips.clips {
        push(&Row::RigClipCandidate { rig: *rig, value })?;
    }
    Ok(())
}

fn weapon_rows(
    capture: &PlayerSourceCapture,
    push: &mut impl FnMut(&Row<'_>) -> io::Result<()>,
) -> io::Result<()> {
    for (index, value) in capture.weapon_motion_sources.weapons.iter().enumerate() {
        push(&Row::WeaponMotionSource { index, value })?;
    }
    for value in &capture.weapon_motion_sources.loadouts {
        push(&Row::WeaponMotionLoadout { value })?;
    }
    for value in &capture.weapon_motion_tags {
        push(&Row::WeaponMotionTag { value })?;
    }
    Ok(())
}
