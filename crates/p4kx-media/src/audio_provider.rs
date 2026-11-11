// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::sound::wwise_path;

use crate::audio_bank_source;
use crate::audio_loose_inventory;
use crate::audio_source_language;
use crate::audio_wem_projection::{self, WemProjection};
use crate::origin_contract::OriginProfile;
use crate::required_reader::RequiredReader;
use crate::{
    Capability, DirectoryContext, FieldGroup, MediaError, MediaKind, MediaWriter, Projection,
    Provider, SourceContext,
};

pub(crate) struct AudioProvider;

pub(crate) fn is_bank(path: &str) -> bool {
    matches!(AudioSource::from_path(path), Some(AudioSource::Bank))
}

impl Provider for AudioProvider {
    fn capability(&self) -> Capability {
        let mut capability = Capability::new(
            MediaKind::Audio,
            "p4kx.audio",
            "1.1.0",
            "Exact decoded bytes of selected WEM and BNK Entries, with ordered original chunk, section, media and object ranges into their retained payloads.",
        );
        capability.field_groups = field_groups();
        capability
    }

    fn directory(
        &self,
        context: &mut DirectoryContext<'_, '_, '_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        audio_loose_inventory::project(context, writer)
    }

    fn directory_complete(
        &self,
        context: &mut DirectoryContext<'_, '_, '_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        crate::audio_foreign::project(context, writer)
    }

    fn source(
        &self,
        context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        let path = context.entry.path();
        let Some(kind) = AudioSource::from_path(path) else {
            return Ok(Projection::Skipped);
        };
        let input = context.origin_input(writer)?;
        let placement = match kind {
            AudioSource::Bank => (wwise_path::bank_folder(path), OriginProfile::AudioBank),
            AudioSource::Wem => (wwise_path::loose_folder(path), OriginProfile::AudioWem),
        };
        if let (Some(folder), profile) = placement {
            audio_source_language::project(&input, folder, profile, writer)?;
        }
        let reader = RequiredReader::open(
            context.payload,
            context.entry.path(),
            context.source.size,
            context.directory.input_state()?.budget,
            writer.checkpoint,
            writer.limits(),
        )?;
        let size = context.source.size;
        match kind {
            AudioSource::Bank => {
                audio_bank_source::project(&input, path, size, context.directory, reader, writer)?;
            }
            AudioSource::Wem => {
                let projection = WemProjection::new(&input, path, size);
                audio_wem_projection::project(&projection, reader, writer)?;
            }
        }
        Ok(Projection::Written)
    }
}

fn field_groups() -> Vec<FieldGroup> {
    wem_field_groups()
        .into_iter()
        .chain(bank_field_groups())
        .chain(bank_node_field_groups())
        .chain([
            FieldGroup::new(
                "loose-wem-inventory",
                "Complete physical numeric loose WEM candidates by owning Archive directory, with original language placement and selection.",
            ),
            FieldGroup::new(
                "source-language",
                "Physical Wwise language-folder placement from selected BNK and loose WEM directory paths.",
            ),
            FieldGroup::new(
                "pcm-wav-conversion",
                "Requested WAV output from complete PCM WEMs, with source frame ranges and verified artifact mappings.",
            ),
            FieldGroup::new(
                "adpcm-wav-conversion",
                "Requested WAV output from IMA ADPCM and PTADPCM, with exact consumed coded blocks and verified artifact mappings.",
            ),
            FieldGroup::new(
                "vorbis-ogg-conversion",
                "Requested Ogg output from Wwise Vorbis, with original packet extents and verified artifact mappings.",
            ),
            FieldGroup::new(
                "vorbis-wav-conversion",
                "Requested WAV output from Wwise Vorbis, with separate checked and decoded original packet ranges.",
            ),
            FieldGroup::new(
                "opus-ogg-conversion",
                "Requested Ogg output from Wwise, Switch and embedded Ogg Opus layouts, with checked and emitted input ranges.",
            ),
            FieldGroup::new(
                "opus-wav-conversion",
                "Requested WAV from Wwise, Switch and embedded Ogg Opus layouts, with exact consumed packet ranges.",
            ),
            FieldGroup::new_with_unsupported_projection(
                "compressed-audio-conversion",
                "Other encoded WEM payloads remain available through exact retained ranges.",
            ),
        ])
        .collect()
}

fn wem_field_groups() -> [FieldGroup; 5] {
    [
        FieldGroup::new(
            "wem-summary",
            "RIFF/RIFX byte order, original fmt, frame count, duration and first fitting forward loop.",
        ),
        FieldGroup::new(
            "wem-chunks",
            "All RIFF chunk headers and exact body and padding ranges, in original order.",
        ),
        FieldGroup::new(
            "opaque-wem-class",
            "Original PLUG or MIDI signature, with exact retained payload.",
        ),
        FieldGroup::new(
            "wem-annotations",
            "Original cue points and LIST subchunks, label and note UTF-8 observations, and text-region headers with original code pages.",
        ),
        FieldGroup::new_with_unsupported_projection(
            "wem-chunk-content",
            "Opaque RIFF bodies, text regions with original encoding declarations and encoded samples remain available in retained ranges.",
        ),
    ]
}

fn bank_node_field_groups() -> [FieldGroup; 2] {
    [
        FieldGroup::new(
            "bank-state-fields",
            "Standalone State properties for versions 128 through 145 and version 154 Sound inline State bindings, groups and values with exact source ranges.",
        ),
        FieldGroup::new(
            "bank-rtpc-fields",
            "Version-154 Sound, ActorMixer and container RTPC curves and points, original float words and interpolation codes with exact source ranges.",
        ),
    ]
}

fn bank_field_groups() -> [FieldGroup; 14] {
    [
        FieldGroup::new(
            "bank-directory",
            "Original physical section order, raw ranges, BKHD values and recognized duplicate diagnostics.",
        ),
        FieldGroup::new(
            "bank-media-index",
            "Original DIDX triplets, complete DATA candidate states, retained media ranges and observed embedded or prefetched media class.",
        ),
        FieldGroup::new(
            "bank-object-directory",
            "Original HIRC object envelopes, raw type codes, IDs, body ranges and section trailing bytes.",
        ),
        FieldGroup::new(
            "bank-action-fields",
            "HIRC Action type and target, original PlayBank words, flags and planar property lists, local Play and PlayEvent candidates, and checked foreign candidates in their owning Archive.",
        ),
        FieldGroup::new(
            "bank-event-fields",
            "HIRC Event count encoding, ordered original action IDs, physical Action candidates and resolution, and exact residual ranges.",
        ),
        FieldGroup::new(
            "bank-container-fields",
            "Original HIRC container parent, kind, mode and ordered children, ActorMixer children, and version-154 shared node properties, FX, auxiliary routing, positioning, advanced settings, State and RTPC fields for containers and ActorMixer. Version-154 Random/Sequence timing, mode codes, flags and ordered signed playlist weights, Switch defaults, ordered memberships and per-child configurations, and Blend layers, per-layer RTPCs, child associations and crossfade graph points.",
        ),
        FieldGroup::new(
            "bank-music-playlist-fields",
            "Original MusicPlaylist node, children, stingers and their scheduling words, transition extent and ordered item fields.",
        ),
        FieldGroup::new(
            "bank-music-segment-fields",
            "Original MusicSegment node, meter, tracks, stingers and their scheduling words, and marker values with exact source ranges.",
        ),
        FieldGroup::new(
            "bank-music-switch-fields",
            "Original MusicSwitch group and physical decision-node words, stinger scheduling words, checked target paths and transition extents.",
        ),
        FieldGroup::new(
            "bank-music-track-fields",
            "Original MusicTrack sources, clips, kind, switch members, automation points, flags, subtrack count, Switch fades and look-ahead with exact source ranges.",
        ),
        FieldGroup::new(
            "bank-sound-fields",
            "Original HIRC Sound source, node property, FX, routing, positioning and advanced playback fields, physical DIDX candidates for embedded and prefetch media, and owning-directory loose WEM references for streamed and prefetch media.",
        ),
        FieldGroup::new(
            "bank-name-index",
            "Original STID header and names, exact bytes, UTF-8 interpretation and residual ranges.",
        ),
        FieldGroup::new(
            "embedded-wem-metadata",
            "WEM summaries, RIFF chunks and cue and LIST annotations in exact complete DIDX ranges.",
        ),
        FieldGroup::new_with_unsupported_projection(
            "bank-payload-content",
            "Other HIRC fields and prefetched or opaque media remain available through exact retained ranges.",
        ),
    ]
}

enum AudioSource {
    Wem,
    Bank,
}

impl AudioSource {
    fn from_path(path: &str) -> Option<Self> {
        let name = path.rsplit(['/', '\\']).next()?;
        let (stem, extension) = name.rsplit_once('.')?;
        if stem.is_empty() {
            return None;
        }
        if extension.eq_ignore_ascii_case("wem") {
            Some(Self::Wem)
        } else if extension.eq_ignore_ascii_case("bnk") {
            Some(Self::Bank)
        } else {
            None
        }
    }
}
