// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use p4kx_extraction::sound::{WemCueText, WemCueTextRegion, WemListPayload, WemTextUtf8};

use crate::audio_wem_annotation_payload::{CueText, CueTextRegion, ListContent, TextValidity};
use crate::audio_wem_projection::WemProjection;
use crate::{MediaError, MediaIssue, Severity, Status};

pub(crate) struct ProjectedListContent<'text> {
    pub(crate) content: ListContent<'text>,
    pub(crate) status: Status,
    pub(crate) issue: Option<MediaIssue>,
}

pub(crate) fn content<'text>(
    projection: &WemProjection<'_>,
    value: &WemListPayload,
    utf8_text: Option<&'text str>,
) -> Result<ProjectedListContent<'text>, MediaError> {
    let content = match value {
        WemListPayload::Label(text) => {
            ListContent::Label(text_record(projection, text, utf8_text)?)
        }
        WemListPayload::Note(text) => ListContent::Note(text_record(projection, text, utf8_text)?),
        WemListPayload::TextRegion(text) => ListContent::TextRegion(text_region(projection, text)?),
        WemListPayload::Opaque => ListContent::Opaque,
        WemListPayload::Malformed { available, needed } => {
            return Ok(short(*available, *needed));
        }
        _ => return Err(MediaError::Contract("unrecognized WEM LIST payload".into())),
    };
    Ok(ProjectedListContent {
        content,
        status: Status::Complete,
        issue: None,
    })
}

fn text_record<'text>(
    projection: &WemProjection<'_>,
    text: &WemCueText,
    utf8_text: Option<&'text str>,
) -> Result<CueText<'text>, MediaError> {
    let utf8 = match text.utf8 {
        WemTextUtf8::Ascii => TextValidity::Ascii,
        WemTextUtf8::Valid => TextValidity::Valid,
        WemTextUtf8::Invalid => TextValidity::Invalid,
        _ => {
            return Err(MediaError::Contract(
                "unrecognized WEM UTF-8 validation".into(),
            ));
        }
    };
    Ok(CueText {
        cue_id: text.cue_id,
        raw_range: projection.range(text.raw_range.start, text.raw_range.end)?,
        text_range: projection.range(text.text_range.start, text.text_range.end)?,
        terminator_offset: text
            .terminator_offset
            .map(|offset| projection.offset(offset))
            .transpose()?,
        utf8_input_range: projection
            .range(text.utf8_input_range.start, text.utf8_input_range.end)?,
        utf8,
        utf8_text,
    })
}

fn text_region(
    projection: &WemProjection<'_>,
    text: &WemCueTextRegion,
) -> Result<CueTextRegion, MediaError> {
    Ok(CueTextRegion {
        cue_id: text.cue_id,
        sample_length: text.sample_length,
        purpose: text.purpose,
        country: text.country,
        language: text.language,
        dialect: text.dialect,
        code_page: text.code_page,
        text_range: projection.range(text.text_range.start, text.text_range.end)?,
    })
}

fn short(available: u64, needed: u64) -> ProjectedListContent<'static> {
    ProjectedListContent {
        content: ListContent::Malformed {
            available: DecimalU64::new(available),
            needed: DecimalU64::new(needed),
        },
        status: Status::Partial,
        issue: Some(MediaIssue::new(
            "audio.wem.short-list-subchunk",
            "LIST",
            Severity::Warning,
            format!("The retained LIST body has {available} bytes and needs {needed}"),
        )),
    }
}
