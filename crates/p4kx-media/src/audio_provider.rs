// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::audio_bank_source;
use crate::audio_wem_projection::{self, WemProjection};
use crate::required_reader::RequiredReader;
use crate::{
    Capability, FieldGroup, MediaError, MediaKind, MediaWriter, Projection, Provider, SourceContext,
};

pub(crate) struct AudioProvider;

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
            AudioSource::Bank => audio_bank_source::project(&input, path, size, reader, writer)?,
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
        .chain([
            FieldGroup::new(
                "pcm-wav-conversion",
                "Requested WAV output from complete PCM WEMs, with source frame ranges and verified artifact mappings.",
            ),
            FieldGroup::new_with_unsupported_projection(
                "compressed-audio-conversion",
                "Compressed WEM payloads remain available through exact retained ranges.",
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
            "HIRC Action type and target, version-gated PlayBank ID and type, and exact original field receipts.",
        ),
        FieldGroup::new(
            "bank-event-fields",
            "HIRC Event count encoding and original words, ordered action IDs, and exact residual ranges.",
        ),
        FieldGroup::new(
            "bank-container-fields",
            "Original HIRC container parent, kind, mode and ordered child IDs with exact byte ranges.",
        ),
        FieldGroup::new(
            "bank-music-playlist-fields",
            "Original MusicPlaylist node, children, stingers, transition extent and ordered item fields.",
        ),
        FieldGroup::new(
            "bank-music-segment-fields",
            "Original MusicSegment node, meter, tracks, stingers and marker values with exact source ranges.",
        ),
        FieldGroup::new(
            "bank-music-switch-fields",
            "Original MusicSwitch group and physical decision-node words, checked target paths and transition extents.",
        ),
        FieldGroup::new(
            "bank-music-track-fields",
            "Original MusicTrack sources, clips, kind and switch members with exact source ranges.",
        ),
        FieldGroup::new(
            "bank-sound-fields",
            "Original HIRC Sound source, storage, cache and flag fields, source plug-in extents and node parent with exact receipts.",
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
