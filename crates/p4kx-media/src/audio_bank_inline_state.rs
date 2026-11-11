// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{
    BankHircField, BankInlineStateRow, BankSoundStateRead, BankStateBinding, BankStateGroup,
    StateProperties,
};

use crate::audio_bank_projection::BankProjection;
use crate::audio_bank_state::{PropertyScope, StateProjection};
use crate::audio_bank_state_payload::{
    InlineStateRecord, StateBindingRecord, StateCountRecord, StateEntryRecord, StateGroupRecord,
    StatePayload,
};
use crate::{MediaError, MediaWriter, Status, Subresource};

pub(crate) const INLINE_NAMESPACE: &str = "audio-bank-hirc-inline-state";
pub(crate) const BINDING_COUNT_NAMESPACE: &str = "audio-bank-hirc-inline-state-binding-count";
pub(crate) const BINDING_NAMESPACE: &str = "audio-bank-hirc-inline-state-binding";
pub(crate) const GROUP_COUNT_NAMESPACE: &str = "audio-bank-hirc-inline-state-group-count";
pub(crate) const GROUP_NAMESPACE: &str = "audio-bank-hirc-inline-state-group";
pub(crate) const ENTRY_NAMESPACE: &str = "audio-bank-hirc-inline-state-entry";

pub(crate) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    state: &StateProjection<'_>,
    status: BankSoundStateRead<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let version = state.version;
    let mut prepared = state.record(writer, None)?;
    let (record, rows) = match status {
        BankSoundStateRead::Readable(rows) => {
            let extent = rows.range();
            state.sites.summary(&mut prepared, Some(extent), false)?;
            state.sites.range(&mut prepared, "checked_range", extent)?;
            (
                InlineStateRecord::Decoded {
                    version,
                    checked_range: extent.into(),
                },
                Some(rows),
            )
        }
        BankSoundStateRead::Absent => {
            state
                .sites
                .summary(&mut prepared, Some(&state.body_range), false)?;
            (InlineStateRecord::Absent { version }, None)
        }
        BankSoundStateRead::Unavailable(_) => {
            state
                .sites
                .summary(&mut prepared, Some(&state.body_range), false)?;
            (InlineStateRecord::Unavailable { version }, None)
        }
        _ => {
            return Err(MediaError::Contract(
                "unexpected inline State disposition".into(),
            ));
        }
    };
    prepared.finish().write(
        writer,
        &Subresource::new(INLINE_NAMESPACE, state.indices.to_vec()),
        Status::Complete,
        &StatePayload::Inline(record),
        &[],
    )?;
    if let Some(rows) = rows {
        for row in rows {
            let row = row.map_err(|cause| projection.replay_failure(cause))?;
            emit_row(state, row, writer)?;
        }
    }
    Ok(())
}

fn emit_row(
    state: &StateProjection<'_>,
    row: BankInlineStateRow,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    match row {
        BankInlineStateRow::BindingCount(count) => {
            emit_count(state, CountKind::Binding, &count, writer)
        }
        BankInlineStateRow::Binding { ordinal, fields } => {
            emit_binding(state, ordinal, &fields, writer)
        }
        BankInlineStateRow::GroupCount(count) => {
            emit_count(state, CountKind::Group, &count, writer)
        }
        BankInlineStateRow::Group { ordinal, fields } => {
            emit_group(state, ordinal, &fields, writer)
        }
        BankInlineStateRow::State {
            group,
            ordinal,
            state_id,
            properties,
        } => {
            let member = StateMember {
                group,
                ordinal,
                state_id,
                properties,
            };
            emit_entry(state, &member, writer)?;
            state.properties(
                &member.properties,
                PropertyScope::Inline {
                    group,
                    state: ordinal,
                },
                writer,
            )
        }
        _ => Err(MediaError::Contract("unexpected inline State row".into())),
    }
}

#[derive(Clone, Copy)]
enum CountKind {
    Binding,
    Group,
}

fn emit_count(
    state: &StateProjection<'_>,
    kind: CountKind,
    count: &BankHircField<u32>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = state.record(writer, None)?;
    let record = StateCountRecord {
        count: count.into(),
    };
    let (namespace, original, payload) = match kind {
        CountKind::Binding => (
            BINDING_COUNT_NAMESPACE,
            "stateBindingCount",
            StatePayload::InlineBindingCount(record),
        ),
        CountKind::Group => (
            GROUP_COUNT_NAMESPACE,
            "stateGroupCount",
            StatePayload::InlineGroupCount(record),
        ),
    };
    state
        .sites
        .field(&mut prepared, "count", &count.range, original)?;
    prepared.finish().write(
        writer,
        &Subresource::new(namespace, state.indices.to_vec()),
        Status::Complete,
        &payload,
        &[],
    )
}

fn emit_binding(
    state: &StateProjection<'_>,
    ordinal: u32,
    fields: &BankStateBinding,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = state.record(writer, Some(&fields.property_id.range))?;
    state.sites.field(
        &mut prepared,
        "property_id",
        &fields.property_id.range,
        "stateBindingPropertyId",
    )?;
    state.sites.field(
        &mut prepared,
        "accumulation",
        &fields.accumulation.range,
        "stateAccumulation",
    )?;
    state
        .sites
        .field(&mut prepared, "in_db", &fields.in_db.range, "stateInDb")?;
    let payload = StatePayload::InlineBinding(StateBindingRecord {
        property_id: (&fields.property_id).into(),
        accumulation: (&fields.accumulation).into(),
        in_db: (&fields.in_db).into(),
    });
    let [section, object] = state.indices;
    prepared.finish().write(
        writer,
        &Subresource::new(BINDING_NAMESPACE, vec![section, object, u64::from(ordinal)]),
        Status::Complete,
        &payload,
        &[],
    )
}

fn emit_group(
    state: &StateProjection<'_>,
    ordinal: u32,
    fields: &BankStateGroup,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = state.record(writer, Some(&fields.group_id.range))?;
    state.sites.field(
        &mut prepared,
        "group_id",
        &fields.group_id.range,
        "stateGroupId",
    )?;
    state.sites.field(
        &mut prepared,
        "synchronization",
        &fields.synchronization.range,
        "stateSynchronization",
    )?;
    state.sites.field(
        &mut prepared,
        "state_count",
        &fields.state_count.range,
        "stateCount",
    )?;
    let payload = StatePayload::InlineGroup(StateGroupRecord {
        group_id: (&fields.group_id).into(),
        synchronization: (&fields.synchronization).into(),
        state_count: (&fields.state_count).into(),
    });
    let [section, object] = state.indices;
    prepared.finish().write(
        writer,
        &Subresource::new(GROUP_NAMESPACE, vec![section, object, u64::from(ordinal)]),
        Status::Complete,
        &payload,
        &[],
    )
}

struct StateMember {
    group: u32,
    ordinal: u32,
    state_id: BankHircField<u32>,
    properties: StateProperties,
}

fn emit_entry(
    state: &StateProjection<'_>,
    member: &StateMember,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = state.record(writer, Some(&member.state_id.range))?;
    let extent = member.properties.consumed_range();
    state
        .sites
        .field(&mut prepared, "state_id", &member.state_id.range, "stateId")?;
    state.property_header(&mut prepared, &member.properties, &extent)?;
    let payload = StatePayload::InlineEntry(StateEntryRecord {
        state_id: (&member.state_id).into(),
        property_count: member.properties.count().into(),
        consumed_range: (&extent).into(),
    });
    let [section, object] = state.indices;
    prepared.finish().write(
        writer,
        &Subresource::new(
            ENTRY_NAMESPACE,
            vec![
                section,
                object,
                u64::from(member.group),
                u64::from(member.ordinal),
            ],
        ),
        Status::Complete,
        &payload,
        &[],
    )
}
