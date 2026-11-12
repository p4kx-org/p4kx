// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::bank::{BankHircField, BankMusicTrackClip, BankMusicTrackSource};

use super::{
    BankOrigins, List, MediaError, MediaOriginInput, MediaWriter, OriginContributions, Phase,
    PreparedRecordOrigins, SourceUseRole, TrackSites, coordinate, field, span, track_site,
};

#[derive(Clone, Copy)]
struct Row {
    list: List,
    indices: [u64; 3],
}

impl Row {
    const fn phase(self) -> Phase {
        match self.list {
            List::Source => Phase::Sources,
            List::Clip => Phase::Clips,
            List::SwitchMember => Phase::SwitchMembers,
        }
    }
}

impl TrackSites<'_> {
    pub(in crate::audio_bank_music_track) fn source(
        &self,
        input: &MediaOriginInput,
        ordinal: u64,
        source: &BankMusicTrackSource,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = BankOrigins::new(input, writer)?;
        let row = self.row(&mut prepared, List::Source, ordinal)?;
        self.source_words(&mut prepared, row, source)?;
        self.source_ranges(&mut prepared, row, source)?;
        Ok(prepared.finish())
    }

    fn source_words(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        row: Row,
        source: &BankMusicTrackSource,
    ) -> Result<(), MediaError> {
        let fields = &source.fields;
        let plugin = fields.plugin_id.as_ref().map(coordinate);
        let storage = fields.storage_code.as_ref().map(coordinate);
        let bits = fields.source_bits.as_ref().map(coordinate);
        for (path, name, bytes) in [
            ("/data/payload/plugin_id", "musicTrackPluginId", plugin),
            ("/data/payload/storage_code", "musicTrackStorage", storage),
            (
                "/data/payload/source_id",
                "musicTrackSourceId",
                fields.source_id.as_ref().map(coordinate),
            ),
            (
                "/data/payload/cache_id",
                "musicTrackSourceCacheId",
                fields.cache_id.as_ref().map(coordinate),
            ),
            (
                "/data/payload/in_memory_size",
                "musicTrackMemorySize",
                fields.in_memory_size.as_ref().map(coordinate),
            ),
            ("/data/payload/source_bits", "musicTrackSourceBits", bits),
        ] {
            let evidence = self.row_evidence(
                prepared,
                row,
                bytes
                    .as_ref()
                    .map(|bytes| field(bytes, name, SourceUseRole::Payload)),
            )?;
            prepared.field(path, evidence)?;
        }
        self.source_derivations(
            prepared,
            row,
            plugin.as_ref(),
            storage.as_ref(),
            bits.as_ref(),
        )
    }

    fn source_derivations(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        row: Row,
        plugin: Option<&[u64; 2]>,
        storage: Option<&[u64; 2]>,
        bits: Option<&[u64; 2]>,
    ) -> Result<(), MediaError> {
        let plugin =
            plugin.map(|bytes| field(bytes, "musicTrackPluginId", SourceUseRole::Conversion));
        let source = self.row_evidence(prepared, row, plugin)?;
        for key in ["source_plugin", "external_source", "midi", "has_media_file"] {
            prepared.field(&format!("/data/payload/{key}"), source.clone())?;
        }
        let storage =
            storage.map(|bytes| field(bytes, "musicTrackStorage", SourceUseRole::Conversion));
        let storage = self.row_evidence(prepared, row, storage)?;
        prepared.field("/data/payload/storage", storage)?;
        let language =
            bits.map(|bytes| field(bytes, "musicTrackSourceBits", SourceUseRole::Conversion));
        let language = self.row_evidence(prepared, row, language)?;
        prepared.field("/data/payload/language_specific", language)
    }

    fn source_ranges(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        row: Row,
        source: &BankMusicTrackSource,
    ) -> Result<(), MediaError> {
        let plugin = source.fields.plugin_id.as_ref().map(coordinate);
        let length = source.plugin_parameter_length.as_ref().map(coordinate);
        let route = track_site(&row.indices, row.list.route());
        let sites: Vec<_> = [
            Some(route),
            plugin
                .as_ref()
                .map(|bytes| field(bytes, "musicTrackPluginId", SourceUseRole::Selector)),
            length.as_ref().map(|bytes| {
                field(
                    bytes,
                    "musicTrackPluginParameterLength",
                    SourceUseRole::Payload,
                )
            }),
        ]
        .into_iter()
        .flatten()
        .collect();
        let evidence = self.evidence(prepared, row.phase(), &sites)?;
        prepared.field("/data/payload/plugin_parameter_length", evidence.clone())?;
        let start = self.row_evidence(prepared, row, None)?;
        prepared.range("/data/payload/source_range", &start, &evidence)?;
        if source.plugin_parameters.is_some() {
            prepared.range(
                "/data/payload/plugin_parameters_range",
                &evidence,
                &evidence,
            )
        } else {
            prepared.field("/data/payload/plugin_parameters_range", evidence)
        }
    }

    pub(in crate::audio_bank_music_track) fn clip(
        &self,
        input: &MediaOriginInput,
        ordinal: u64,
        clip: &BankMusicTrackClip,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = BankOrigins::new(input, writer)?;
        let row = self.row(&mut prepared, List::Clip, ordinal)?;
        for (path, name, bytes) in [
            (
                "/data/payload/track_id",
                "musicTrackClipTrackId",
                Some(span(&clip.track_id.range)),
            ),
            (
                "/data/payload/source_id",
                "musicTrackClipSourceId",
                Some(span(&clip.source_id.range)),
            ),
            (
                "/data/payload/cache_id",
                "musicTrackClipCacheId",
                clip.cache_id.as_ref().map(coordinate),
            ),
            (
                "/data/payload/event_id",
                "musicTrackClipEventId",
                Some(span(&clip.event_id.range)),
            ),
            (
                "/data/payload/play_at",
                "musicTrackClipPlayAt",
                Some(span(&clip.play_at.range)),
            ),
            (
                "/data/payload/begin_trim",
                "musicTrackClipBeginTrim",
                Some(span(&clip.begin_trim.range)),
            ),
            (
                "/data/payload/end_trim",
                "musicTrackClipEndTrim",
                Some(span(&clip.end_trim.range)),
            ),
            (
                "/data/payload/source_duration",
                "musicTrackClipSourceDuration",
                Some(span(&clip.source_duration.range)),
            ),
        ] {
            let evidence = self.row_evidence(
                &prepared,
                row,
                bytes
                    .as_ref()
                    .map(|bytes| field(bytes, name, SourceUseRole::Payload)),
            )?;
            prepared.subtree(path, evidence)?;
        }
        let extent = self.row_evidence(&prepared, row, None)?;
        prepared.range("/data/payload/clip_range", &extent, &extent)?;
        Ok(prepared.finish())
    }

    pub(in crate::audio_bank_music_track) fn switch_member(
        &self,
        input: &MediaOriginInput,
        ordinal: u64,
        member: &BankHircField<u32>,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = BankOrigins::new(input, writer)?;
        let row = self.row(&mut prepared, List::SwitchMember, ordinal)?;
        let bytes = span(&member.range);
        let evidence = self.row_evidence(
            &prepared,
            row,
            Some(field(
                &bytes,
                "musicTrackSwitchValueId",
                SourceUseRole::Payload,
            )),
        )?;
        prepared.field("/data/payload/value_id", evidence.clone())?;
        prepared.range("/data/payload/value_id_range", &evidence, &evidence)?;
        Ok(prepared.finish())
    }

    fn row(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        list: List,
        ordinal: u64,
    ) -> Result<Row, MediaError> {
        let row = Row {
            list,
            indices: self.indices(ordinal),
        };
        let evidence = self.row_evidence(prepared, row, None)?;
        self.object.indices_origin(prepared, evidence.clone())?;
        let object = prepared.capture([self.object.route()])?;
        prepared.field("/subresource/indices/1", object)?;
        prepared.field("/data/payload/ordinal", evidence)?;
        Ok(row)
    }

    fn row_evidence(
        &self,
        prepared: &BankOrigins<'_, '_, '_>,
        row: Row,
        word: Option<super::EvidenceSite<'_>>,
    ) -> Result<OriginContributions, MediaError> {
        let route = track_site(&row.indices, row.list.route());
        word.map_or_else(
            || self.evidence(prepared, row.phase(), &[route]),
            |word| self.evidence(prepared, row.phase(), &[route, word]),
        )
    }
}
