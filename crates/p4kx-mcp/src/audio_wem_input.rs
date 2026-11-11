// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{AudioIndex, MAX_SOUNDBANK_LEN, WemLocation};
use p4kx_extraction::sound::WemOriginPlan;
use p4kx_extraction::{
    Archive, AudioError, Bank, Codec, ConversionError, EmbeddedWem, ObservedEntry, Sound,
    SourceInput, Wem,
};

use crate::archives::Opened;
use crate::audio_convert;
use crate::audio_params::MAX_NAME_LEN;
use crate::audio_shapes::LocationShape;
use crate::audio_wem_shapes::WemShape;
use crate::audio_window::{Kept, Window};
use crate::error::{ToolError, capped};

/// What the header of a WEM states about the sound.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Facts {
    pub(crate) codec: &'static str,
    pub(crate) channels: u16,
    pub(crate) sample_rate: u32,
    pub(crate) sample_count: Option<u64>,
}

pub(crate) struct WavInput {
    pub(crate) codec: Codec,
    pub(crate) data_offset: u64,
    pub(crate) layout_present: bool,
}

/// What the tools do with a parsed WEM, whichever buffer holds its bytes.
pub(crate) trait WemOps {
    /// Describes the WEM.
    fn shape(&self, id: Option<u32>, size: u64) -> WemShape;
    /// Returns what the header states.
    fn facts(&self) -> Facts;
    /// Returns the original field coordinates of this WEM parse.
    fn origin_plan(&self) -> WemOriginPlan<'_>;
    /// Returns the encoded WEM body supplied to the decoder.
    fn wav_input(&self) -> WavInput;
    /// Decodes the sound into `window`, which keeps its frames; `limit` is the cap it was given.
    fn window(&self, name: &str, window: Window, limit: u64) -> Result<Kept, ToolError>;
}

impl<B: AsRef<[u8]>> WemOps for Wem<B> {
    fn shape(&self, id: Option<u32>, size: u64) -> WemShape {
        WemShape {
            id,
            size,
            prefetch: false,
            codec: Some(self.codec().id()),
            codec_name: Some(self.codec().to_string()),
            channels: Some(self.channels()),
            sample_rate: Some(self.sample_rate()),
            sample_count: self.sample_count(),
            duration_seconds: self.duration().map(|duration| duration.as_secs_f64()),
            looping: Some(self.loop_points().is_some()),
            plugin_media: false,
            error: None,
        }
    }

    fn facts(&self) -> Facts {
        Facts {
            codec: self.codec().id(),
            channels: self.channels(),
            sample_rate: self.sample_rate(),
            sample_count: self.sample_count(),
        }
    }

    fn origin_plan(&self) -> WemOriginPlan<'_> {
        WemOriginPlan::new(self.info_source(), self.codec(), self.sample_count())
    }

    fn wav_input(&self) -> WavInput {
        WavInput {
            codec: self.codec(),
            data_offset: self.info_source().data_body_offset(),
            layout_present: self.metadata().channel_layout().is_some(),
        }
    }

    fn window(&self, name: &str, mut window: Window, limit: u64) -> Result<Kept, ToolError> {
        match self.write_wav(&mut window) {
            Ok(_) => {}
            Err(_) if window.is_full() => {}
            Err(_) if window.is_exceeded() => return Err(ToolError::ExcerptSource { limit }),
            Err(source) => return Err(audio_error(name, source)),
        }
        window.into_kept().ok_or_else(|| {
            audio_error(
                name,
                AudioError::InvalidFormat("the decoded WAV has no fmt chunk"),
            )
        })
    }
}

pub(crate) fn audio_error(name: &str, source: AudioError) -> ToolError {
    ToolError::Convert {
        path: name.to_owned(),
        what: "a sound to decode",
        source: Box::new(ConversionError::Audio {
            sound: name.to_owned(),
            source,
        }),
    }
}

/// Where the bytes of a WEM are read from.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Source {
    /// A loose WEM Entry.
    Entry(usize),
    /// A WEM embedded in the soundbank Entry.
    Embedded {
        position: usize,
        media: EmbeddedSite,
    },
}

/// An exact embedded WEM selection in its bank's decoded DATA section.
#[derive(Debug, Clone, Copy)]
pub(crate) struct EmbeddedSite {
    pub(crate) id: u32,
    pub(crate) offset: u64,
    pub(crate) len: u64,
}

impl EmbeddedSite {
    const fn matches(self, wem: &EmbeddedWem<'_>) -> bool {
        wem.id() == self.id && wem.offset() == self.offset && wem.bytes().len() as u64 == self.len
    }
}

/// A place that holds a WEM.
#[derive(Debug, Clone)]
pub(crate) struct Candidate {
    pub(crate) source: Source,
    pub(crate) location: LocationShape,
    language: LanguageSource,
}

#[derive(Debug, Clone, Copy)]
enum LanguageSource {
    SelectedPath,
    IndexEntry(usize),
    IncompleteIndex,
}

/// Original input of the displayed language placement.
#[derive(Debug, Clone, Copy)]
pub(crate) enum LanguageInput<'a> {
    SelectedPath,
    IndexEntry(ObservedEntry<'a>),
    IncompleteIndex,
}

impl LanguageSource {
    fn observed(self, archive: &Archive) -> LanguageInput<'_> {
        match self {
            Self::SelectedPath => LanguageInput::SelectedPath,
            Self::IndexEntry(position) => archive
                .entries()
                .get(position)
                .and_then(|entry| archive.observed_entry(entry))
                .map_or(LanguageInput::IncompleteIndex, LanguageInput::IndexEntry),
            Self::IncompleteIndex => LanguageInput::IncompleteIndex,
        }
    }
}

/// The places that hold the WEM `params` names, loose Entries first, with its ID where there is
/// one.
pub(crate) fn candidates(
    opened: &Opened,
    wem_id: Option<u32>,
    path: Option<&str>,
) -> Result<(Option<u32>, Vec<Candidate>), ToolError> {
    match (wem_id, path) {
        (Some(id), None) => {
            let index = opened.audio_index();
            let places: Vec<Candidate> = index
                .loose_wem(id)
                .chain(index.embedded_wem(id))
                .filter_map(|place| indexed_candidate(&index, place, id))
                .collect();
            if places.is_empty() {
                return Err(ToolError::UnknownWem(id));
            }
            Ok((Some(id), places))
        }
        (None, Some(path)) => {
            let path = capped("path", path, MAX_NAME_LEN * 4)?;
            let index = opened.index();
            let entry = index
                .find(path)
                .ok_or_else(|| ToolError::UnknownEntry(path.to_owned()))?;
            let stem = entry.path().rsplit(['/', '\\']).next().and_then(|name| {
                let cut = name.len().checked_sub(".wem".len())?;
                name.get(cut..)
                    .is_some_and(|ending| ending.eq_ignore_ascii_case(".wem"))
                    .then(|| name.get(..cut))
                    .flatten()
            });
            let (language_name, language) = folder_language(&opened.audio_index(), entry.path());
            let location = LocationShape {
                kind: "entry",
                path: entry.path().to_owned(),
                language: language_name,
                offset: None,
                len: entry.uncompressed_size(),
            };
            Ok((
                stem.and_then(|stem| stem.parse().ok()),
                vec![Candidate {
                    source: Source::Entry(entry.index()),
                    location,
                    language,
                }],
            ))
        }
        _ => Err(ToolError::WemSelector),
    }
}

fn indexed_candidate(index: &AudioIndex, place: WemLocation<'_>, id: u32) -> Option<Candidate> {
    let source = match place {
        WemLocation::Entry { entry, .. } => Source::Entry(usize::try_from(entry).ok()?),
        WemLocation::Soundbank {
            soundbank,
            offset,
            len,
        } => Source::Embedded {
            position: soundbank.entry_position(),
            media: EmbeddedSite { id, offset, len },
        },
        _ => return None,
    };
    let location = audio_convert::location(&place)?;
    let language = match (source, location.language.as_deref()) {
        (Source::Entry(_), Some(name)) => index_language(index, name),
        _ => LanguageSource::SelectedPath,
    };
    Some(Candidate {
        source,
        location,
        language,
    })
}

fn index_language(index: &AudioIndex, name: &str) -> LanguageSource {
    index
        .language_entry_position(name)
        .map_or(LanguageSource::IncompleteIndex, LanguageSource::IndexEntry)
}

fn folder_language(index: &AudioIndex, path: &str) -> (Option<String>, LanguageSource) {
    let mut folders = path.split(['\\', '/']);
    if !folders.any(|folder| folder.eq_ignore_ascii_case("wwise")) {
        return (None, LanguageSource::SelectedPath);
    }
    let mut rest = folders.collect::<Vec<_>>();
    rest.pop();
    if rest.is_empty() {
        return (None, LanguageSource::SelectedPath);
    }
    for folder in rest {
        if let Some(name) = index.language(folder) {
            return (Some(name.to_owned()), index_language(index, name));
        }
    }
    (None, LanguageSource::IncompleteIndex)
}

/// What reading a place found.
pub(crate) enum Held<'a> {
    /// A WEM that parsed.
    Parsed(ParsedWem<'a>),
    /// The prefetched start of a streamed WEM.
    Prefetch,
    /// Bytes that are no sound.
    Unreadable(AudioError),
}

pub(crate) struct ParsedWem<'a> {
    pub(crate) wem: &'a dyn WemOps,
    pub(crate) source: SourceInput<'a>,
    pub(crate) embedded: Option<EmbeddedSite>,
    pub(crate) language: LanguageInput<'a>,
}

/// Reads the WEM at `candidate` and hands what was found to `visit`.
pub(crate) fn read<R>(
    opened: &Opened,
    candidate: &Candidate,
    visit: impl FnOnce(Held<'_>) -> Result<R, ToolError>,
) -> Result<R, ToolError> {
    let archive = opened.archive();
    let path = &candidate.location.path;
    let language = candidate.language.observed(archive);
    match candidate.source {
        Source::Entry(position) => {
            let sound = Sound::from_archive_position_observed(archive, position)
                .map_err(|error| ToolError::converted(path, "a WEM", error))?;
            visit(Held::Parsed(ParsedWem {
                wem: sound.wem(),
                source: sound.source_input().unwrap_or(SourceInput::Unknown),
                embedded: None,
                language,
            }))
        }
        Source::Embedded { position, media } => {
            if let Some(entry) = archive.entries().get(position)
                && entry.uncompressed_size() > MAX_SOUNDBANK_LEN
            {
                return Err(ToolError::SoundbankTooLarge {
                    path: entry.path().to_owned(),
                    limit: MAX_SOUNDBANK_LEN,
                });
            }
            let bank = Bank::from_archive_position_observed(archive, position)
                .map_err(|error| ToolError::converted(path, "a soundbank", error))?;
            let soundbank = bank.soundbank();
            let wem = soundbank
                .wem(media.id)
                .filter(|wem| media.matches(wem))
                .or_else(|| soundbank.wems().find(|wem| media.matches(wem)))
                .ok_or(ToolError::UnknownWem(media.id))?;
            if wem.is_prefetch() {
                return visit(Held::Prefetch);
            }
            match wem.parse() {
                Ok(parsed) => visit(Held::Parsed(ParsedWem {
                    wem: &parsed,
                    source: bank
                        .archive_origin()
                        .map_or(SourceInput::Unknown, SourceInput::Archive),
                    embedded: Some(media),
                    language,
                })),
                Err(error) => visit(Held::Unreadable(error)),
            }
        }
    }
}
