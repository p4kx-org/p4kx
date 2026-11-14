// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::sound::{WemFormatField, WemOriginField, WemOriginPlan, WemOriginSite};
use p4kx_extraction::{
    Codec, FieldOrigin, InputScope, SourceCoordinateOmission, SourceEvidenceError, SourceInput,
    SourceInventory, SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_wem_input::{EmbeddedSite, LanguageInput, ParsedWem};

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExcerptFieldOrigins {
    #[serde(rename = "/structuredContent/archive")]
    archive: FieldOrigin,
    #[serde(rename = "/structuredContent/wem_id")]
    wem_id: FieldOrigin,
    #[serde(rename = "/structuredContent/source/kind")]
    source_kind: FieldOrigin,
    #[serde(rename = "/structuredContent/source/path")]
    source_path: FieldOrigin,
    #[serde(rename = "/structuredContent/source/language")]
    source_language: FieldOrigin,
    #[serde(
        rename = "/structuredContent/source/offset",
        skip_serializing_if = "Option::is_none"
    )]
    source_offset: Option<FieldOrigin>,
    #[serde(rename = "/structuredContent/source/len")]
    source_len: FieldOrigin,
    #[serde(rename = "/structuredContent/codec")]
    codec: FieldOrigin,
    #[serde(rename = "/structuredContent/mime_type")]
    mime_type: FieldOrigin,
    #[serde(rename = "/structuredContent/channels")]
    channels: FieldOrigin,
    #[serde(rename = "/structuredContent/sample_rate")]
    sample_rate: FieldOrigin,
    #[serde(rename = "/structuredContent/sound_seconds")]
    sound_seconds: FieldOrigin,
    #[serde(rename = "/structuredContent/start_seconds")]
    start_seconds: FieldOrigin,
    #[serde(rename = "/structuredContent/duration_seconds")]
    duration_seconds: FieldOrigin,
    #[serde(rename = "/structuredContent/frames")]
    frames: FieldOrigin,
    #[serde(rename = "/structuredContent/bytes")]
    bytes: FieldOrigin,
    #[serde(rename = "/structuredContent/max_bytes")]
    max_bytes: FieldOrigin,
    #[serde(rename = "/structuredContent/truncated")]
    truncated: FieldOrigin,
    #[serde(rename = "/content/1/data")]
    wav: FieldOrigin,
}

pub(crate) struct ExcerptPcm {
    pub(crate) retained: Range<u64>,
    pub(crate) observed_len: u64,
}

impl ExcerptFieldOrigins {
    pub(crate) fn capture(
        parsed: &ParsedWem<'_>,
        id_from_path: bool,
        pcm: ExcerptPcm,
    ) -> Result<(Self, SourceInventory), SourceEvidenceError> {
        let evidence = AudioEvidence::observe(parsed)?;
        let path = evidence.directory("path")?;
        let local = evidence.uses.origin(&[], true);
        let source_path = evidence.origin(std::slice::from_ref(&path));
        let source_kind = evidence.source_kind(&path)?;
        let source_language = evidence.language(path, parsed.language)?;
        let source_len = evidence.source_len()?;
        let source_offset = evidence.source_offset()?;
        let playback = PlaybackOrigins::capture(&evidence, parsed, pcm)?;
        let fields = Self {
            archive: local.clone(),
            wem_id: if id_from_path {
                source_path.clone()
            } else {
                local.clone()
            },
            source_kind,
            source_path,
            source_language,
            source_offset,
            source_len,
            codec: playback.codec,
            mime_type: local.clone(),
            channels: playback.channels,
            sample_rate: playback.rate.clone(),
            sound_seconds: playback.sound,
            start_seconds: playback.rate,
            duration_seconds: playback.frames.clone(),
            frames: playback.frames.clone(),
            bytes: playback.wav.clone(),
            max_bytes: local,
            truncated: playback.frames,
            wav: playback.wav,
        };
        Ok((fields, evidence.uses.freeze()?))
    }
}

struct PlaybackOrigins {
    codec: FieldOrigin,
    channels: FieldOrigin,
    rate: FieldOrigin,
    frames: FieldOrigin,
    sound: FieldOrigin,
    wav: FieldOrigin,
}

impl PlaybackOrigins {
    fn capture(
        evidence: &AudioEvidence,
        parsed: &ParsedWem<'_>,
        pcm: ExcerptPcm,
    ) -> Result<Self, SourceEvidenceError> {
        let plan = parsed.wem.origin_plan();
        let codec = evidence.field(&plan, WemOriginField::Codec)?;
        let channels = evidence.field(&plan, WemOriginField::Channels)?;
        let rate = evidence.field(&plan, WemOriginField::SampleRate)?;
        let mut common = evidence.field(&plan, WemOriginField::RiffByteOrder)?;
        common.extend(codec.iter().cloned());
        common.extend(channels.iter().cloned());
        common.extend(rate.iter().cloned());
        let input = parsed.wem.wav_input();
        if matches!(input.codec, Codec::ImaAdpcm | Codec::PtAdpcm) {
            common
                .extend(evidence.field(&plan, WemOriginField::Format(WemFormatField::BlockAlign))?);
        }
        let mut total = common.clone();
        total.extend(evidence.field(&plan, WemOriginField::SampleCount)?);
        if pcm.observed_len != 0 {
            total.push(evidence.decoder_input(
                input.codec,
                input.data_offset,
                0..pcm.observed_len,
                "pcmExtent",
            )?);
        }
        if !pcm.retained.is_empty() {
            common.push(evidence.decoder_input(
                input.codec,
                input.data_offset,
                pcm.retained,
                "pcmSamples",
            )?);
        }
        let frames = evidence.origin(&common);
        if parsed.wem.facts().channels > 2 {
            common.extend(evidence.field(
                &plan,
                WemOriginField::Format(WemFormatField::ChannelMask {
                    layout_present: input.layout_present,
                }),
            )?);
        }
        Ok(Self {
            codec: evidence.origin(&codec),
            channels: evidence.origin(&channels),
            rate: evidence.origin(&rate),
            frames,
            sound: evidence.origin(&total),
            wav: evidence.origin(&common),
        })
    }
}

struct AudioEvidence {
    uses: SourceUses,
    scope: SourceScope,
    embedded: Option<EmbeddedSite>,
    complete: bool,
}

impl AudioEvidence {
    fn observe(parsed: &ParsedWem<'_>) -> Result<Self, SourceEvidenceError> {
        let uses = SourceUses::new();
        let scope = uses.observe(parsed.source, InputScope::Payload)?;
        let complete = match parsed.source {
            SourceInput::Archive(origin) => origin.ancestors().all(|step| step.is_some()),
            _ => false,
        };
        Ok(Self {
            uses,
            scope,
            embedded: parsed.embedded,
            complete,
        })
    }

    fn origin(&self, uses: &[SourceUse]) -> FieldOrigin {
        self.uses.origin(uses, self.complete)
    }

    fn directory(&self, field: &'static str) -> Result<SourceUse, SourceEvidenceError> {
        self.scope
            .for_scope(InputScope::ArchiveDirectoryMetadata)
            .at(
                SourceLocation::Metadata {
                    field: field.into(),
                },
                SourceUseRole::Payload,
            )
    }

    fn language(
        &self,
        path: SourceUse,
        input: LanguageInput<'_>,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        match input {
            LanguageInput::SelectedPath => Ok(self.origin(&[path])),
            LanguageInput::IndexEntry(entry) => {
                let canonical = self.uses.observe(
                    SourceInput::ArchiveView(entry),
                    InputScope::ArchiveDirectoryMetadata,
                )?;
                let spelling = canonical.at(
                    SourceLocation::Metadata {
                        field: "path".into(),
                    },
                    SourceUseRole::Selector,
                )?;
                let complete = self.complete && entry.ancestors().all(|step| step.is_some());
                Ok(self.uses.origin(&[path, spelling], complete))
            }
            LanguageInput::IncompleteIndex => Ok(self.uses.origin(&[path], false)),
        }
    }

    fn decoder_input(
        &self,
        codec: Codec,
        offset: u64,
        pcm: Range<u64>,
        samples: &'static str,
    ) -> Result<SourceUse, SourceEvidenceError> {
        if codec == Codec::Pcm {
            self.wem_site(
                "wemDecodedEntryBytes",
                &[offset + pcm.start, offset + pcm.end],
                samples,
                SourceUseRole::Conversion,
            )
        } else {
            self.wem_site(
                "wemChunk",
                &[offset],
                "decodedAudioInput",
                SourceUseRole::Conversion,
            )
        }
    }

    fn source_kind(&self, path: &SourceUse) -> Result<FieldOrigin, SourceEvidenceError> {
        match self.embedded {
            Some(media) => {
                let selector = self.bank_member(media, "didxSelection", SourceUseRole::Selector)?;
                Ok(self.origin(&[selector]))
            }
            None => Ok(self.origin(std::slice::from_ref(path))),
        }
    }

    fn source_len(&self) -> Result<FieldOrigin, SourceEvidenceError> {
        let length = match self.embedded {
            Some(site) => self.bank_member(site, "size", SourceUseRole::Payload)?,
            None => self.directory("uncompressed_size")?,
        };
        Ok(self.origin(&[length]))
    }

    fn source_offset(&self) -> Result<Option<FieldOrigin>, SourceEvidenceError> {
        self.embedded
            .map(|site| {
                self.bank_member(site, "offset", SourceUseRole::Payload)
                    .map(|input| self.origin(&[input]))
            })
            .transpose()
    }

    fn bank_member(
        &self,
        media: EmbeddedSite,
        name: &'static str,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        self.scope.at(
            SourceLocation::Subresource {
                namespace: "bnkEmbeddedWem".into(),
                indices: vec![u64::from(media.id), media.offset, media.len],
                field: name.into(),
            },
            role,
        )
    }

    fn field(
        &self,
        plan: &WemOriginPlan<'_>,
        field: WemOriginField,
    ) -> Result<Vec<SourceUse>, SourceEvidenceError> {
        let mut actual = Vec::new();
        if let Some(media) = self.embedded {
            actual.push(self.bank_member(media, "didxSelection", SourceUseRole::Selector)?);
        }
        match plan.field(field) {
            Ok(sites) => {
                for site in sites {
                    actual.push(self.bind(site)?);
                }
            }
            Err(_) => actual.push(self.scope.at_opaque(
                SourceUseRole::Conversion,
                SourceCoordinateOmission::Unavailable,
            )?),
        }
        Ok(actual)
    }

    fn bind(&self, site: WemOriginSite) -> Result<SourceUse, SourceEvidenceError> {
        if self.embedded.is_none() {
            site.bind(&self.scope)
        } else {
            self.wem_site(site.namespace(), site.indices(), site.name(), site.role())
        }
    }

    fn wem_site(
        &self,
        namespace: &'static str,
        indices: &[u64],
        field: &'static str,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        let (namespace, indices) = self.embedded.map_or_else(
            || (namespace.into(), indices.to_vec()),
            |media| {
                let mut bank_indices = vec![u64::from(media.id), media.offset, media.len];
                bank_indices.extend_from_slice(indices);
                (format!("bnkEmbeddedWem.{namespace}"), bank_indices)
            },
        );
        self.scope.at(
            SourceLocation::Subresource {
                namespace,
                indices,
                field: field.into(),
            },
            role,
        )
    }
}
