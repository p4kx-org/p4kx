// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::bank::{
    BankNodeFxKind, BankNodeFxSlots, BankNodePropertyKind, BankNodePropertyRows,
    BankNodePropertyValue, BankNodeUserAuxBuses, BankPositioningRowFields, BankPositioningRows,
    BankPositioningTableKind, BankPositioningVector,
};
use p4kx_media::MediaRecord;

use super::{ADVANCED, AUX, Common, Input, Output, Owner, POSITIONING, PROPERTIES, TestResult};

pub(super) const PROPERTY_LISTS: [BankNodePropertyKind; 2] = [
    BankNodePropertyKind::Properties,
    BankNodePropertyKind::RangedModifiers,
];
pub(super) const EFFECT_LISTS: [BankNodeFxKind; 2] =
    [BankNodeFxKind::Effects, BankNodeFxKind::Metadata];
pub(super) const POSITIONING_TABLES: [BankPositioningTableKind; 3] = [
    BankPositioningTableKind::Vertex,
    BankPositioningTableKind::Playlist,
    BankPositioningTableKind::AutomationRange,
];

pub(super) fn common(output: &Output<'_>, owner: Owner, source: Common<'_>) -> TestResult {
    let row = output.row(PROPERTIES, &owner)?;
    output.field(
        row,
        "/payload/observed/override_bus_id",
        source.parent.override_bus_id(),
    )?;
    output.field(
        row,
        "/payload/observed/parent_id",
        source.parent.parent_id.as_ref(),
    )?;
    output.field(
        row,
        "/payload/observed/priority_flags",
        source.parent.priority_flags.as_ref(),
    )?;
    for (name, kind) in [
        (
            "/payload/observed/properties/declared_count",
            BankNodePropertyKind::Properties,
        ),
        (
            "/payload/observed/ranged_modifiers/declared_count",
            BankNodePropertyKind::RangedModifiers,
        ),
    ] {
        output.field(
            row,
            name,
            source
                .properties
                .get(kind)
                .map(|extent| &extent.declared_count),
        )?;
    }
    let row = output.row(AUX, &owner)?;
    output.field(row, "/payload/observed/flags", source.aux.flags.as_ref())?;
    output.field(
        row,
        "/payload/observed/reflections_bus_id",
        source.aux.reflections_bus_id.as_ref(),
    )?;
    playback(output, owner, &source)?;
    effect_headers(output, owner, &source)
}

fn effect_headers(output: &Output<'_>, owner: Owner, source: &Common<'_>) -> TestResult {
    let [section, object] = owner;
    for (index, kind) in EFFECT_LISTS.into_iter().enumerate() {
        let original = source.parent.fx(kind);
        let row = output.row(
            "audio-bank-hirc-node-fx",
            &[section, object, u64::try_from(index)?],
        )?;
        output.field(
            row,
            "/payload/observed/declared_count",
            original.declared_count,
        )?;
        output.field(
            row,
            "/payload/observed/override_parent_bits",
            original.fields.override_parent_bits.as_ref(),
        )?;
        output.field(
            row,
            "/payload/observed/bypass_all_bits",
            original.fields.bypass_all_bits.as_ref(),
        )?;
    }
    Ok(())
}

fn playback(output: &Output<'_>, owner: Owner, source: &Common<'_>) -> TestResult {
    let original = &source.playback.positioning;
    let row = output.row(POSITIONING, &owner)?;
    for (path, word) in [
        ("/payload/observed/flags", original.flags.as_ref()),
        (
            "/payload/observed/spatial_flags",
            original.spatial_flags.as_ref(),
        ),
        ("/payload/observed/path_mode", original.path_mode.as_ref()),
    ] {
        output.field(row, path, word)?;
    }
    for (path, word) in [
        (
            "/payload/observed/vertex_count",
            original.vertex_count.as_ref(),
        ),
        (
            "/payload/observed/playlist_count",
            original.playlist_count.as_ref(),
        ),
    ] {
        output.field(row, path, word)?;
    }
    output.field(
        row,
        "/payload/observed/transition_time",
        original.transition_time.as_ref(),
    )?;
    let row = output.row(ADVANCED, &owner)?;
    let original = &source.playback.advanced;
    for (path, word) in [
        ("/payload/observed/flags", original.flags.as_ref()),
        (
            "/payload/observed/virtual_queue_behavior",
            original.virtual_queue_behavior.as_ref(),
        ),
        (
            "/payload/observed/below_threshold_behavior",
            original.below_threshold_behavior.as_ref(),
        ),
        (
            "/payload/observed/behavior_flags",
            original.behavior_flags.as_ref(),
        ),
    ] {
        output.field(row, path, word)?;
    }
    output.field(
        row,
        "/payload/observed/max_instances",
        original.max_instances.as_ref(),
    )
}

pub(super) fn properties(
    output: &Output<'_>,
    [section, object]: Owner,
    list: u64,
    rows: Option<BankNodePropertyRows<'_, Input<'_>>>,
) -> TestResult {
    for original in rows.into_iter().flatten() {
        let original = original?;
        let row = output.row(
            "audio-bank-hirc-node-property",
            &[section, object, list, u64::from(original.ordinal)],
        )?;
        output.field(row, "/payload/property_id", Some(&original.id))?;
        match original.value {
            BankNodePropertyValue::Scalar(bits) => {
                output.field(row, "/payload/words/scalar_bits", bits.as_ref())?;
            }
            BankNodePropertyValue::Ranged {
                minimum, maximum, ..
            } => {
                output.field(row, "/payload/words/minimum_bits", minimum.as_ref())?;
                output.field(row, "/payload/words/maximum_bits", maximum.as_ref())?;
            }
            _ => return Err("unexpected original property shape".into()),
        }
    }
    Ok(())
}

pub(super) fn effects(
    output: &Output<'_>,
    [section, object]: Owner,
    list: u64,
    rows: Option<BankNodeFxSlots<'_, Input<'_>>>,
) -> TestResult {
    for original in rows.into_iter().flatten() {
        let original = original?;
        let row = output.row(
            "audio-bank-hirc-node-fx-slot",
            &[section, object, list, u64::from(original.ordinal)],
        )?;
        output.field(row, "/payload/slot_index", Some(&original.index))?;
        output.field(row, "/payload/effect_id", original.effect_id.as_ref())?;
        output.field(row, "/payload/slot_bits", original.bits.as_ref())?;
    }
    Ok(())
}

pub(super) fn paths(
    output: &Output<'_>,
    [section, object]: Owner,
    table: u64,
    rows: Option<BankPositioningRows<'_, Input<'_>>>,
) -> TestResult {
    for original in rows.into_iter().flatten() {
        let original = original?;
        let row = output.row(
            "audio-bank-hirc-node-positioning-path",
            &[section, object, table, u64::from(original.ordinal)],
        )?;
        match original.fields {
            BankPositioningRowFields::Vertex {
                coordinates,
                duration,
                ..
            } => {
                vector(output, row, &coordinates)?;
                output.field(row, "/payload/fields/duration", duration.as_ref())?;
            }
            BankPositioningRowFields::Playlist {
                vertices_offset,
                vertex_count,
                ..
            } => {
                output.field(
                    row,
                    "/payload/fields/vertices_offset",
                    Some(&vertices_offset),
                )?;
                output.field(row, "/payload/fields/vertex_count", vertex_count.as_ref())?;
            }
            BankPositioningRowFields::AutomationRange(coordinates) => {
                vector(output, row, &coordinates)?;
            }
            _ => return Err("unexpected original positioning row".into()),
        }
    }
    Ok(())
}

fn vector(output: &Output<'_>, row: &MediaRecord, vector: &BankPositioningVector) -> TestResult {
    for (path, word) in [
        ("/payload/fields/x/bits", Some(&vector.x_bits)),
        ("/payload/fields/y/bits", vector.y_bits.as_ref()),
        ("/payload/fields/z/bits", vector.z_bits.as_ref()),
    ] {
        output.field(row, path, word)?;
    }
    Ok(())
}

pub(super) fn aux(
    output: &Output<'_>,
    owner: Owner,
    buses: Option<BankNodeUserAuxBuses>,
) -> TestResult {
    if let Some(buses) = buses {
        let row = output.row(AUX, &owner)?;
        for (slot, original) in buses.iter().enumerate() {
            output.field(
                row,
                &format!("/payload/observed/user_bus_ids/{slot}"),
                original.as_ref(),
            )?;
        }
    }
    Ok(())
}
