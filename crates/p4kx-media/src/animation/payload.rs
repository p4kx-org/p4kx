// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use schemars::JsonSchema;
use serde::Serialize;

use super::binding_payload::{Binding, Skeleton};
use super::image_payload::{ImageIndex, ImageRecord};
use crate::{ByteOrder, ChunkReference, OffsetBasis, SourceRange};

pub(crate) const ENTRY: &str = "animation-entry";
pub(crate) const CHUNK: &str = "animation-chunk";
pub(crate) const CLIP: &str = "animation-clip";
pub(crate) const DBA_METADATA: &str = "animation-dba-metadata";
pub(crate) const IMAGE_INDEX: &str = "animation-image-index";
pub(crate) const IMAGE_RECORD: &str = "animation-image-record";
pub(crate) const CONTROLLER: &str = "animation-controller";
pub(crate) const SKELETON: &str = "animation-skeleton";
pub(crate) const BINDING: &str = "animation-controller-binding";

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "AnimationFamily")]
pub(crate) enum Family {
    Caf,
    Dba,
    Aim,
    CafImage,
    DirectionalBlendImage,
}

impl Family {
    pub(crate) fn of(path: &str) -> Option<Self> {
        let (_, extension) = path.rsplit_once('.')?;
        if extension.eq_ignore_ascii_case("img") {
            return Self::of_image(path);
        }
        if extension.eq_ignore_ascii_case("caf") {
            Some(Self::Caf)
        } else if extension.eq_ignore_ascii_case("dba") {
            Some(Self::Dba)
        } else if extension.eq_ignore_ascii_case("aim") {
            Some(Self::Aim)
        } else {
            None
        }
    }

    fn of_image(path: &str) -> Option<Self> {
        let mut parts = path.split(['/', '\\']);
        if !parts.next()?.eq_ignore_ascii_case("data")
            || !parts.next()?.eq_ignore_ascii_case("animations")
        {
            return None;
        }
        let name = parts.next()?;
        if parts.next().is_some() {
            return None;
        }
        if name.eq_ignore_ascii_case("Animations.img") {
            Some(Self::CafImage)
        } else if name.eq_ignore_ascii_case("DirectionalBlends.img") {
            Some(Self::DirectionalBlendImage)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "AnimationEntryLayout")]
pub(crate) enum EntryLayout {
    ClipInventory,
    OpaqueAim,
    ImageIndex,
    InvalidContainer,
}

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(crate) enum AnimationPayload<'a> {
    Entry(Entry),
    Chunk(AnimationChunk),
    Clip(Clip<'a>),
    DbaMetadata(DbaMetadata<'a>),
    ImageIndex(ImageIndex),
    ImageRecord(ImageRecord<'a>),
    Controller(Controller),
    Skeleton(Skeleton<'a>),
    ControllerBinding(Binding),
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationEntry")]
pub(crate) struct Entry {
    pub(crate) family: Family,
    pub(crate) layout: EntryLayout,
    pub(crate) chunk_count: Option<usize>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AnimationChunk {
    pub(crate) type_id: u32,
    pub(crate) type_name: Option<&'static str>,
    pub(crate) version: u32,
    pub(crate) chunk_id: Option<u32>,
    pub(crate) byte_order: ByteOrder,
    pub(crate) range: SourceRange,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationClip")]
pub(crate) struct Clip<'a> {
    pub(crate) metadata_chunk_version: u32,
    pub(crate) data_chunk_version: u32,
    pub(crate) metadata_range: SourceRange,
    #[schemars(length(min = 48, max = 48))]
    pub(crate) metadata_bytes: &'a [u8],
    pub(crate) controller_block_range: SourceRange,
    pub(crate) flags: u32,
    pub(crate) timing: Option<Timing>,
    pub(crate) database_name: Option<DatabaseName<'a>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationDbaMetadata")]
pub(crate) struct DbaMetadata<'a> {
    pub(crate) metadata_chunk_version: u32,
    pub(crate) metadata_range: SourceRange,
    #[schemars(length(min = 48, max = 48))]
    pub(crate) metadata_bytes: &'a [u8],
    pub(crate) flags: u32,
    pub(crate) timing: Option<Timing>,
    pub(crate) database_name: DatabaseName<'a>,
    pub(crate) controller_disposition: ControllerDisposition,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "AnimationControllerDisposition")]
pub(crate) enum ControllerDisposition {
    Unavailable,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationDatabaseName")]
pub(crate) struct DatabaseName<'a> {
    pub(crate) utf8: Option<&'a str>,
    pub(crate) range: SourceRange,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationTiming")]
pub(crate) struct Timing {
    pub(crate) frames_per_second: u16,
    pub(crate) end_frame: u32,
    pub(crate) additive: bool,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationController")]
pub(crate) struct Controller {
    #[serde(rename = "controller_id")]
    pub(crate) id: u32,
    pub(crate) id_range: SourceRange,
    pub(crate) table_range: SourceRange,
    pub(crate) rotation: Channel,
    pub(crate) translation: Channel,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationChannel")]
pub(crate) struct Channel {
    pub(crate) key_count: u16,
    pub(crate) packed_format: u16,
    pub(crate) time_offset: u32,
    pub(crate) value_offset: u32,
    pub(crate) samples: Samples,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "layout", rename_all = "kebab-case", deny_unknown_fields)]
#[schemars(rename = "AnimationSamples")]
pub(crate) enum Samples {
    Empty,
    Decoded(Box<DecodedSamples>),
    Unsupported,
    Malformed,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationDecodedSamples")]
pub(crate) struct DecodedSamples {
    pub(crate) frames: ChunkReference,
    pub(crate) values: ChunkReference,
}

pub(crate) const fn entry_range(range: Range<u64>) -> SourceRange {
    SourceRange::new(OffsetBasis::Entry, range.start, range.end)
}
