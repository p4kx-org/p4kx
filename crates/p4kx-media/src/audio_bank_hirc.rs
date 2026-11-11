// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use p4kx_extraction::DecimalU64;
use p4kx_extraction::bank::{
    BankHircObject, BankHircSemantic, BankObservedHeader, BankSource, BankSourceSection,
};

use crate::audio_bank_failure::BankPhase;
use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_payload::{BankEntryRange, BankHircRecord, BankObjectRecord, BankPayload};
use crate::audio_bank_projection::BankProjection;
use crate::{
    MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource, audio_bank_hirc_action,
    audio_bank_hirc_container, audio_bank_hirc_event, audio_bank_hirc_origins,
    audio_bank_hirc_sound,
};
use crate::{audio_bank_music_segment, audio_bank_music_track};

pub(crate) const HIRC_NAMESPACE: &str = "audio-bank-hirc";
pub(crate) const OBJECT_NAMESPACE: &str = "audio-bank-object";

pub(crate) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    source: &mut BankSource<R>,
    section: &BankSourceSection,
    header: Option<&BankObservedHeader>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut hierarchy = match source.hirc(section) {
        Ok(hierarchy) => hierarchy,
        Err(cause) => {
            return projection.failure(BankPhase::Hierarchy, Some(section), cause, writer);
        }
    };
    let version = header.map(|header| VersionSite {
        version: header.header().version,
        body_offset: header.section().body_offset(),
    });
    loop {
        let next = match header {
            Some(header) => hierarchy.next_semantic(header),
            None => hierarchy
                .next()
                .map(|result| result.map(BankHircSemantic::Raw)),
        };
        let Some(result) = next else { break };
        let semantic = match result {
            Ok(semantic) => semantic,
            Err(cause) => {
                return projection.failure(BankPhase::Hierarchy, Some(section), cause, writer);
            }
        };
        emit_object(projection, semantic.object(), version, writer)?;
        emit_semantic(projection, semantic, version, writer)?;
    }
    let trailing = hierarchy.trailing_range().ok_or_else(|| {
        MediaError::Contract("completed HIRC cursor has no trailing range".into())
    })?;
    emit_summary(
        projection,
        section,
        hierarchy.object_count(),
        trailing,
        writer,
    )
}

fn emit_semantic<R: Read + Seek>(
    projection: &BankProjection<'_>,
    semantic: BankHircSemantic<'_, R>,
    version: Option<VersionSite>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    match (semantic, version) {
        (BankHircSemantic::Raw(_), _) => Ok(()),
        (BankHircSemantic::Action(action), Some(version)) => audio_bank_hirc_action::project(
            projection,
            &action.object,
            version,
            action.status,
            writer,
        ),
        (BankHircSemantic::Container(container), Some(version)) => {
            audio_bank_hirc_container::project(projection, version, *container, writer)
        }
        (BankHircSemantic::MusicSegment(segment), Some(version)) => {
            audio_bank_music_segment::project(projection, version, *segment, writer)
        }
        (BankHircSemantic::MusicTrack(track), Some(version)) => {
            audio_bank_music_track::project(projection, version, *track, writer)
        }
        (BankHircSemantic::Event(event), Some(version)) => {
            audio_bank_hirc_event::project(projection, version, event, writer)
        }
        (BankHircSemantic::Sound(sound), Some(version)) => {
            audio_bank_hirc_sound::project(projection, version, &sound, writer)
        }
        _ => Err(MediaError::Contract(
            "HIRC semantics have no observed bank version".into(),
        )),
    }
}

fn emit_object(
    projection: &BankProjection<'_>,
    object: &BankHircObject,
    version: Option<VersionSite>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let payload = BankPayload::BankObject(BankObjectRecord {
        section_ordinal: DecimalU64::new(object.section_ordinal()),
        ordinal: DecimalU64::new(object.ordinal()),
        type_code: object.type_code(),
        kind: version.map(|version| object.kind(version.version).slug()),
        object_id: object.id(),
        header_range: BankEntryRange::new(object.header_offset(), object.body_offset()),
        body_range: BankEntryRange::new(object.body_offset(), object.body_end_exclusive()),
    });
    audio_bank_hirc_origins::object(projection.input, object, version, writer)?.write(
        writer,
        &Subresource::new(
            OBJECT_NAMESPACE,
            vec![object.section_ordinal(), object.ordinal()],
        ),
        Status::Complete,
        &payload,
        &[],
    )
}

fn emit_summary(
    projection: &BankProjection<'_>,
    section: &BankSourceSection,
    object_count: u32,
    trailing: Range<u64>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let has_trailing = !trailing.is_empty();
    let payload = BankPayload::BankHirc(BankHircRecord {
        section_ordinal: DecimalU64::new(section.ordinal()),
        declared_objects: object_count,
        trailing_range: BankEntryRange::new(trailing.start, trailing.end),
    });
    let issue = has_trailing.then(|| {
        MediaIssue::new(
            "audio.bank.trailing-hirc-bytes",
            "HIRC",
            Severity::Info,
            format!(
                "{} original bytes follow the declared objects",
                trailing.end - trailing.start
            ),
        )
    });
    audio_bank_hirc_origins::summary(projection.input, section, has_trailing, writer)?.write(
        writer,
        &Subresource::new(HIRC_NAMESPACE, vec![section.ordinal()]),
        Status::Complete,
        &payload,
        issue.as_slice(),
    )
}
