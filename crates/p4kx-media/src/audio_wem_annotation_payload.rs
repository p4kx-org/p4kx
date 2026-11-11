// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use schemars::JsonSchema;
use serde::Serialize;

use crate::SourceRange;
use crate::audio_payload::WemFailureReason;

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(crate) enum AnnotationPayload<'text> {
    #[serde(rename = "wem-cue")]
    Cue(CueHeader),
    #[serde(rename = "wem-cue-point")]
    Point(CuePoint),
    #[serde(rename = "wem-cue-tail")]
    CueTail(CueTail),
    #[serde(rename = "wem-list")]
    List(ListHeader),
    #[serde(rename = "wem-list-item")]
    Item(&'text ListItem<'text>),
    #[serde(rename = "wem-annotation-failure")]
    Failure(AnnotationFailure),
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CueHeader {
    pub(crate) chunk_ordinal: DecimalU64,
    pub(crate) declared_points: u32,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CuePoint {
    pub(crate) chunk_ordinal: DecimalU64,
    pub(crate) ordinal: DecimalU64,
    pub(crate) point_range: SourceRange,
    pub(crate) id: u32,
    pub(crate) position: u32,
    pub(crate) target_chunk: [u8; 4],
    pub(crate) chunk_start: u32,
    pub(crate) block_start: u32,
    pub(crate) sample_offset: u32,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CueTail {
    pub(crate) chunk_ordinal: DecimalU64,
    pub(crate) trailing_range: SourceRange,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListHeader {
    pub(crate) chunk_ordinal: DecimalU64,
    pub(crate) list_type: [u8; 4],
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListItem<'text> {
    pub(crate) chunk_ordinal: DecimalU64,
    pub(crate) ordinal: DecimalU64,
    pub(crate) tag: [u8; 4],
    pub(crate) header_range: SourceRange,
    pub(crate) body_range: SourceRange,
    pub(crate) padding_range: Option<SourceRange>,
    pub(crate) content: ListContent<'text>,
}

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(crate) enum ListContent<'text> {
    Label(CueText<'text>),
    Note(CueText<'text>),
    TextRegion(CueTextRegion),
    Opaque,
    Malformed {
        available: DecimalU64,
        needed: DecimalU64,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CueText<'text> {
    pub(crate) cue_id: u32,
    pub(crate) raw_range: SourceRange,
    pub(crate) text_range: SourceRange,
    pub(crate) terminator_offset: Option<u64>,
    pub(crate) utf8_input_range: SourceRange,
    pub(crate) utf8: TextValidity,
    pub(crate) utf8_text: Option<&'text str>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum TextValidity {
    Ascii,
    Valid,
    Invalid,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CueTextRegion {
    pub(crate) cue_id: u32,
    pub(crate) sample_length: u32,
    pub(crate) purpose: [u8; 4],
    pub(crate) country: u16,
    pub(crate) language: u16,
    pub(crate) dialect: u16,
    pub(crate) code_page: u16,
    pub(crate) text_range: SourceRange,
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AnnotationPhase {
    Cue,
    List,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AnnotationFailure {
    pub(crate) chunk_ordinal: DecimalU64,
    pub(crate) phase: AnnotationPhase,
    pub(crate) reason: WemFailureReason,
}
