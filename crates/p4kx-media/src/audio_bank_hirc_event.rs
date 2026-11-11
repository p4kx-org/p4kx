// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use p4kx_extraction::DecimalU64;
use p4kx_extraction::bank::{
    BankEvent, BankEventAction, BankEventRead, BankHircEvent, BankHircField,
};

use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_payload::BankEntryRange;
use crate::audio_bank_projection::BankProjection;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

#[path = "audio_bank_hirc_event_origins.rs"]
mod origins;
#[path = "audio_bank_hirc_event_payload.rs"]
mod payload;

pub(crate) use self::payload::EventPayload;
use self::payload::{EventActionRecord, EventRecord, EventTail};

pub(crate) const EVENT_NAMESPACE: &str = "audio-bank-hirc-event";
pub(crate) const ACTION_NAMESPACE: &str = "audio-bank-hirc-event-action";
pub(crate) const TAIL_NAMESPACE: &str = "audio-bank-hirc-event-tail";

pub(crate) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    version: VersionSite,
    event: BankHircEvent<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let original = &event.object;
    let sites = origins::EventSites::new(original, version);
    match event.status {
        BankEventRead::Decoded(actions) => decoded(projection, &sites, actions, writer),
        BankEventRead::Malformed {
            encoding,
            count,
            count_prefix,
            error,
        } => {
            let payload = EventRecord::Malformed {
                action_count: count.as_ref().map(|count| count.value),
                count_encoding: encoding.try_into()?,
                count_range: entry_range(&count_prefix),
            };
            let issue = MediaIssue::new(
                "audio.bank.invalid-event",
                "HIRC",
                Severity::Error,
                error.to_string(),
            );
            header(
                projection,
                &sites,
                payload,
                origins::CountRead::Observed {
                    count: count.as_ref(),
                    prefix: &count_prefix,
                },
                &[issue],
                writer,
            )
        }
        BankEventRead::Unsupported { error } => header(
            projection,
            &sites,
            EventRecord::Unsupported {
                action_count: None,
                count_encoding: None,
                count_range: None,
            },
            origins::CountRead::Unsupported,
            &[MediaIssue::new(
                "audio.bank.unsupported-event",
                "HIRC",
                Severity::Info,
                error.to_string(),
            )],
            writer,
        ),
        _ => Err(MediaError::Contract(
            "unexpected HIRC Event reader state".into(),
        )),
    }
}

fn decoded<R: Read + Seek>(
    projection: &BankProjection<'_>,
    sites: &origins::EventSites,
    mut actions: BankEvent<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let count = actions.declared_count().clone();
    header(
        projection,
        sites,
        EventRecord::Decoded {
            action_count: count.value,
            count_encoding: actions.encoding().try_into()?,
            count_range: entry_range(&count.range),
        },
        origins::CountRead::Observed {
            count: Some(&count),
            prefix: &count.range,
        },
        &[],
        writer,
    )?;
    for action in actions.by_ref() {
        let action = action.map_err(|cause| projection.replay_failure(cause))?;
        emit_action(projection, sites, &count, &action, writer)?;
    }
    let trailing = actions.trailing_range().ok_or_else(|| {
        MediaError::Contract("completed HIRC Event cursor has no trailing range".into())
    })?;
    tail(projection, sites, &count, &trailing, writer)
}

fn header(
    projection: &BankProjection<'_>,
    sites: &origins::EventSites,
    record: EventRecord,
    count: origins::CountRead<'_>,
    issues: &[MediaIssue],
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let prepared = sites.header(projection.input, count, !issues.is_empty(), writer)?;
    let status = if matches!(record, EventRecord::Malformed { .. }) {
        Status::Partial
    } else {
        Status::Complete
    };
    prepared.write(
        writer,
        &Subresource::new(EVENT_NAMESPACE, sites.object_indices().to_vec()),
        status,
        &EventPayload::Event(record),
        issues,
    )
}

fn emit_action(
    projection: &BankProjection<'_>,
    sites: &origins::EventSites,
    count: &BankHircField<u32>,
    action: &BankEventAction,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let indices = sites.action_indices(action.ordinal);
    let prepared = sites.action(projection.input, count, action, writer)?;
    let payload = EventPayload::Action(EventActionRecord {
        ordinal: DecimalU64::new(u64::from(action.ordinal)),
        action_id: action.action_id.value,
        id_range: entry_range(&action.action_id.range),
    });
    prepared.write(
        writer,
        &Subresource::new(ACTION_NAMESPACE, indices.to_vec()),
        Status::Complete,
        &payload,
        &[],
    )
}

fn tail(
    projection: &BankProjection<'_>,
    sites: &origins::EventSites,
    count: &BankHircField<u32>,
    trailing: &Range<u64>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let prepared = sites.tail(projection.input, count, writer)?;
    prepared.write(
        writer,
        &Subresource::new(TAIL_NAMESPACE, sites.object_indices().to_vec()),
        Status::Complete,
        &EventPayload::Tail(EventTail {
            trailing_range: entry_range(trailing),
        }),
        &[],
    )
}

const fn entry_range(range: &Range<u64>) -> BankEntryRange {
    BankEntryRange::new(range.start, range.end)
}
