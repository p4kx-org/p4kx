// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::bank::{
    BankSoundContinuation, BankSoundFields, BankSoundNodeBase, MediaStorage, SoundSource,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::MediaError;
use crate::audio_bank_hirc_sound::loose::{LooseWemLink, LooseWemResolution};
use crate::audio_bank_payload::BankEntryRange;
use crate::audio_bank_sound_reference::{SoundMediaCandidate, SoundMediaResolution};

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(crate) enum SoundPayload {
    BankHircSound(SoundRecord),
    #[serde(rename = "bank-hirc-sound-media-resolution")]
    Resolution(SoundMediaResolution),
    #[serde(rename = "bank-hirc-sound-media-candidate")]
    Candidate(SoundMediaCandidate),
    #[serde(rename = "bank-hirc-sound-loose-wem-resolution")]
    LooseResolution(LooseWemResolution),
    #[serde(rename = "bank-hirc-sound-loose-wem-candidate")]
    LooseCandidate(LooseWemLink),
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "layout", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum SoundRecord {
    Decoded {
        source: SourceRecord,
        continuation: Continuation,
    },
    Malformed {
        source: SourcePrefix,
        continuation: Continuation,
    },
    Unsupported {
        source: Option<()>,
        continuation: Option<()>,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Independent Wwise flag projections are separate schema fields"
)]
pub(crate) struct SourceRecord {
    plugin_id: u32,
    storage_code: u8,
    storage: Storage,
    source_id: u32,
    cache_id: Option<u32>,
    in_memory_size: u32,
    source_bits: Option<u8>,
    language_specific: bool,
    source_plugin: bool,
    external_source: bool,
    midi: bool,
    has_media_file: bool,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Storage {
    Embedded,
    PrefetchStreamed,
    Streamed,
    Other,
}

impl From<MediaStorage> for Storage {
    fn from(value: MediaStorage) -> Self {
        match value {
            MediaStorage::Embedded => Self::Embedded,
            MediaStorage::PrefetchStreamed => Self::PrefetchStreamed,
            MediaStorage::Streamed => Self::Streamed,
            MediaStorage::Other(_) => Self::Other,
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourcePrefix {
    plugin_id: Option<u32>,
    storage_code: Option<u8>,
    source_id: Option<u32>,
    cache_id: Option<u32>,
    in_memory_size: Option<u32>,
    source_bits: Option<u8>,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum Continuation {
    Unreached,
    Opaque {
        opaque_range: BankEntryRange,
    },
    Absent,
    Node {
        plugin_parameter_length: Option<u32>,
        plugin_parameters_range: Option<BankEntryRange>,
        node_base: Option<NodeBase>,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct NodeBase {
    effect_count: Option<u8>,
    metadata_count: Option<u8>,
    pre_parent_word: Option<u32>,
    parent_id: Option<u32>,
    consumed_range: BankEntryRange,
    trailing_range: Option<BankEntryRange>,
}

impl SoundRecord {
    pub(crate) fn decoded(
        source: SoundSource,
        fields: &BankSoundFields,
    ) -> Result<Self, MediaError> {
        let storage_code = fields
            .storage_code
            .as_ref()
            .map(|field| field.value)
            .ok_or_else(|| {
                MediaError::Contract("decoded Sound has no original storage byte".into())
            })?;
        Ok(Self::Decoded {
            source: SourceRecord {
                plugin_id: source.plugin_id,
                storage_code,
                storage: Storage::from(source.storage),
                source_id: source.source_id,
                cache_id: fields.cache_id.as_ref().map(|field| field.value),
                in_memory_size: source.in_memory_size,
                source_bits: fields.source_bits.as_ref().map(|field| field.value),
                language_specific: source.is_language_specific(),
                source_plugin: source.is_source_plugin(),
                external_source: source.is_external_source(),
                midi: source.is_midi(),
                has_media_file: source.has_media_file(),
            },
            continuation: Continuation::read(fields)?,
        })
    }

    pub(crate) fn malformed(fields: &BankSoundFields) -> Result<Self, MediaError> {
        Ok(Self::Malformed {
            source: SourcePrefix {
                plugin_id: fields.plugin_id.as_ref().map(|field| field.value),
                storage_code: fields.storage_code.as_ref().map(|field| field.value),
                source_id: fields.source_id.as_ref().map(|field| field.value),
                cache_id: fields.cache_id.as_ref().map(|field| field.value),
                in_memory_size: fields.in_memory_size.as_ref().map(|field| field.value),
                source_bits: fields.source_bits.as_ref().map(|field| field.value),
            },
            continuation: Continuation::read(fields)?,
        })
    }

    pub(crate) const fn unsupported() -> Self {
        Self::Unsupported {
            source: None,
            continuation: None,
        }
    }
}

impl Continuation {
    fn read(fields: &BankSoundFields) -> Result<Self, MediaError> {
        match &fields.continuation {
            BankSoundContinuation::Unreached => Ok(Self::Unreached),
            BankSoundContinuation::Opaque { range } => Ok(Self::Opaque {
                opaque_range: entry_range(range),
            }),
            BankSoundContinuation::Absent => Ok(Self::Absent),
            BankSoundContinuation::Node => {
                let node = fields.node.as_ref().ok_or_else(|| {
                    MediaError::Contract("Sound node selection has no original reads".into())
                })?;
                Ok(Self::Node {
                    plugin_parameter_length: node
                        .plugin_parameter_length
                        .as_ref()
                        .map(|field| field.value),
                    plugin_parameters_range: node.plugin_parameters.as_ref().map(entry_range),
                    node_base: node.base.as_ref().map(NodeBase::read),
                })
            }
            _ => Err(MediaError::Contract("unexpected Sound continuation".into())),
        }
    }
}

impl NodeBase {
    fn read(node: &BankSoundNodeBase) -> Self {
        Self {
            effect_count: node.fields.effect_count.as_ref().map(|field| field.value),
            metadata_count: node.fields.metadata_count.as_ref().map(|field| field.value),
            pre_parent_word: node
                .fields
                .pre_parent_word
                .as_ref()
                .map(|field| field.value),
            parent_id: node.fields.parent_id.as_ref().map(|field| field.value),
            consumed_range: entry_range(&node.consumed),
            trailing_range: node.trailing.as_ref().map(entry_range),
        }
    }
}

const fn entry_range(range: &Range<u64>) -> BankEntryRange {
    BankEntryRange::new(range.start, range.end)
}
