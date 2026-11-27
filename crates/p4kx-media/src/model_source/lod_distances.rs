// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::chunks::{ChunkDirectory, ivo};
use schemars::JsonSchema;
use serde::Serialize;

use super::origins::{self, ModelOrigins};
use super::payload::ModelSourcePayload;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::required_reader::RequiredReader;
use crate::{MediaError, MediaWriter, Status, Subresource};

pub(super) const NAMESPACE: &str = "model-lod-distance-words";
const MEASURED_VERSION: u32 = 0x900;
const WORDS: usize = 12;
const WORD_BYTES: usize = size_of::<u32>();
const MEASURED_BYTES: u64 = (WORDS * WORD_BYTES) as u64;

#[derive(Serialize, JsonSchema)]
#[serde(tag = "disposition", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum LodDistanceWords {
    Words { words: [LodDistanceWord; WORDS] },
    UnsupportedVersion,
    UnmeasuredExtent,
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct LodDistanceWord {
    bits: u32,
    finite_binary32: Option<f32>,
}

impl LodDistanceWord {
    fn new(bytes: [u8; WORD_BYTES]) -> Self {
        let bits = u32::from_le_bytes(bytes);
        let value = f32::from_bits(bits);
        Self {
            bits,
            finite_binary32: value.is_finite().then_some(value),
        }
    }
}

pub(super) fn emit(
    file: &ChunkDirectory,
    index: usize,
    reader: &mut RequiredReader<'_>,
    origins: &ModelOrigins<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let missing = || MediaError::Contract("LOD distance chunk lost its original table slot".into());
    let chunk = file.chunks().get(index).ok_or_else(missing)?;
    if chunk.chunk_type() != ivo::LOD_DISTANCES {
        return Ok(());
    }
    let row = file.table_row(index).ok_or_else(missing)?;
    let mut prepared = origins.prepared()?;
    let version = origins.payload(
        row.version,
        "modelChunkVersion",
        SourceUseRole::Selector,
        writer,
    )?;
    let extent = origins::chunk_extent(file.table_range(), row.offset, row.size, origins, writer)?;
    let mut layout = writer.capture_origins();
    layout.include_original(&version)?;
    layout.include_original(&extent.end)?;
    let data = if chunk.version() != MEASURED_VERSION {
        LodDistanceWords::UnsupportedVersion
    } else if chunk.size() != MEASURED_BYTES {
        LodDistanceWords::UnmeasuredExtent
    } else {
        let words = read_words(chunk.offset(), reader, origins, &mut prepared, writer)?;
        LodDistanceWords::Words { words }
    };
    origins::at(
        &mut prepared,
        "/data/payload/disposition",
        layout.finish(),
        writer,
    )?;
    prepared.write(
        writer,
        &Subresource::new(NAMESPACE, vec![index as u64]),
        Status::Complete,
        &ModelSourcePayload::LodDistanceWords(data),
        &[],
    )
}

fn read_words(
    start: u64,
    reader: &mut RequiredReader<'_>,
    origins: &ModelOrigins<'_>,
    prepared: &mut PreparedRecordOrigins,
    writer: &MediaWriter<'_>,
) -> Result<[LodDistanceWord; WORDS], MediaError> {
    let end = start + MEASURED_BYTES;
    let bytes = reader.read_chunk(start..end)?;
    let words: &[[u8; WORD_BYTES]; WORDS] = bytes
        .as_chunks::<WORD_BYTES>()
        .0
        .try_into()
        .map_err(|_| MediaError::Contract("admitted LOD word count changed".into()))?;
    let all = origins.payload(
        start..end,
        "lodDistanceWords",
        SourceUseRole::Payload,
        writer,
    )?;
    origins::at(prepared, "/data/payload/words", all, writer)?;
    for ordinal in 0..WORDS {
        let word = start + ordinal as u64 * WORD_BYTES as u64;
        let use_word = origins.payload(
            word..word + WORD_BYTES as u64,
            "lodDistanceWord",
            SourceUseRole::Payload,
            writer,
        )?;
        origins::at(
            prepared,
            &format!("/data/payload/words/{ordinal}"),
            use_word,
            writer,
        )?;
    }
    Ok(words.map(LodDistanceWord::new))
}
