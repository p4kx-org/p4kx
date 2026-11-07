// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{AudioIndex, MAX_SOUNDBANK_LEN};
use p4kx_extraction::{AudioError, Bank, ConversionError, EmbeddedWem, Sound, Wem};

use crate::archives::Opened;
use crate::audio_convert as convert;
use crate::audio_params::{MAX_NAME_LEN, SoundbankParams, WemParams};
use crate::audio_shapes::LocationShape;
use crate::audio_wem_shapes::{SoundbankWemsResult, WemInfoResult, WemShape};
use crate::audio_window::{Kept, Window};
use crate::budget::{fill_within, json_len};
use crate::error::{ToolError, capped};
use crate::page::Page;

/// The most places read, one after the other, to find one that holds the whole WEM.
pub(crate) const MAX_ATTEMPTS: usize = 8;

/// What the header of a WEM states about the sound.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Facts {
    pub(crate) codec: &'static str,
    pub(crate) channels: u16,
    pub(crate) sample_rate: u32,
    pub(crate) sample_count: Option<u64>,
}

/// What the tools do with a parsed WEM, whichever buffer holds its bytes.
pub(crate) trait WemOps {
    /// Describes the WEM.
    fn shape(&self, id: Option<u32>, size: u64) -> WemShape;
    /// Returns what the header states.
    fn facts(&self) -> Facts;
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

/// A WEM that cannot be parsed as a sound.
fn unreadable(id: Option<u32>, size: u64, error: Option<&AudioError>) -> WemShape {
    let plugin = matches!(error, Some(AudioError::PluginMedia));
    WemShape {
        id,
        size,
        prefetch: error.is_none(),
        codec: None,
        codec_name: None,
        channels: None,
        sample_rate: None,
        sample_count: None,
        duration_seconds: None,
        looping: None,
        plugin_media: plugin,
        error: error.filter(|_| !plugin).map(ToString::to_string),
    }
}

fn embedded_shape(wem: &EmbeddedWem<'_>) -> WemShape {
    let size = wem.bytes().len() as u64;
    let id = Some(wem.id());
    if wem.is_prefetch() {
        return unreadable(id, size, None);
    }
    match wem.parse() {
        Ok(parsed) => parsed.shape(id, size),
        Err(error) => unreadable(id, size, Some(&error)),
    }
}

/// Returns one page of the WEMs the soundbank at `params.soundbank` embeds, each described by its
/// header.
pub(crate) fn soundbank_wems(
    opened: &Opened,
    params: &SoundbankParams,
    page: Page,
) -> Result<SoundbankWemsResult, ToolError> {
    let path = capped("soundbank", &params.soundbank, MAX_NAME_LEN * 4)?;
    let bank = open_bank(opened, path)?;
    let soundbank = bank.soundbank();
    let wems: Vec<EmbeddedWem<'_>> = soundbank.wems().collect();
    let items = page.slice(&wems).iter().map(embedded_shape);
    let mut result = SoundbankWemsResult {
        archive: opened.archive().path().display().to_string(),
        soundbank: bank.name().to_owned(),
        version: soundbank.header().version,
        bank_id: soundbank.header().id,
        total: wems.len(),
        wems: Vec::new(),
        next_cursor: None,
    };
    let (listed, next_cursor) = fill_within(
        page,
        wems.len(),
        json_len(&result),
        items.map(Ok),
        |wem: &WemShape| format!("WEM {}", wem.id.unwrap_or_default()),
    )?;
    result.wems = listed;
    result.next_cursor = next_cursor;
    Ok(result)
}

/// Reads the soundbank Entry at `path`, refusing one larger than the audio index reads.
pub(crate) fn open_bank(opened: &Opened, path: &str) -> Result<Bank, ToolError> {
    if let Some(entry) = opened.index().find(path)
        && entry.uncompressed_size() > MAX_SOUNDBANK_LEN
    {
        return Err(ToolError::SoundbankTooLarge {
            path: entry.path().to_owned(),
            limit: MAX_SOUNDBANK_LEN,
        });
    }
    Bank::from_archive(opened.archive(), path)
        .map_err(|error| ToolError::converted(path, "a soundbank", error))
}

/// Where the bytes of a WEM are read from.
#[derive(Debug, Clone)]
pub(crate) enum Source {
    /// A loose WEM Entry.
    Entry(String),
    /// A WEM embedded in the soundbank Entry.
    Embedded {
        /// The Entry path of the soundbank.
        bank: String,
        /// The WEM ID.
        id: u32,
    },
}

/// A place that holds a WEM.
#[derive(Debug, Clone)]
pub(crate) struct Candidate {
    pub(crate) source: Source,
    pub(crate) location: LocationShape,
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
                .filter_map(|place| convert::location(&place))
                .map(|location| Candidate {
                    source: Source::Entry(location.path.clone()),
                    location,
                })
                .chain(
                    index
                        .embedded_wem(id)
                        .filter_map(|place| convert::location(&place))
                        .map(|location| Candidate {
                            source: Source::Embedded {
                                bank: location.path.clone(),
                                id,
                            },
                            location,
                        }),
                )
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
            let location = LocationShape {
                kind: "entry",
                path: entry.path().to_owned(),
                language: folder_language(&opened.audio_index(), entry.path()),
                offset: None,
                len: entry.uncompressed_size(),
            };
            Ok((
                stem.and_then(|stem| stem.parse().ok()),
                vec![Candidate {
                    source: Source::Entry(location.path.clone()),
                    location,
                }],
            ))
        }
        _ => Err(ToolError::WemSelector),
    }
}

/// Returns the language folder of the Archive that the Entry `path` lies in below
/// `Data\Sounds\wwise\`, spelled as the Archive spells it, or `None` for an Entry outside any
/// language folder.
fn folder_language(index: &AudioIndex, path: &str) -> Option<String> {
    let mut folders = path.split(['\\', '/']);
    folders.find(|folder| folder.eq_ignore_ascii_case("wwise"))?;
    let mut rest = folders.collect::<Vec<_>>();
    rest.pop();
    rest.into_iter()
        .find_map(|folder| index.language(folder))
        .map(str::to_owned)
}

/// What reading a place found.
pub(crate) enum Held<'a> {
    /// A WEM that parsed.
    Parsed(&'a dyn WemOps),
    /// The prefetched start of a streamed WEM.
    Prefetch,
    /// Bytes that are no sound.
    Unreadable(AudioError),
}

/// Reads the WEM at `candidate` and hands what was found to `visit`.
pub(crate) fn read<R>(
    opened: &Opened,
    candidate: &Candidate,
    visit: impl FnOnce(Held<'_>) -> Result<R, ToolError>,
) -> Result<R, ToolError> {
    match &candidate.source {
        Source::Entry(path) => {
            let sound = Sound::from_archive(opened.archive(), path)
                .map_err(|error| ToolError::converted(path, "a WEM", error))?;
            visit(Held::Parsed(sound.wem()))
        }
        Source::Embedded { bank, id } => {
            let held = open_bank(opened, bank)?;
            let Some(wem) = held.soundbank().wem(*id) else {
                return Err(ToolError::UnknownWem(*id));
            };
            if wem.is_prefetch() {
                return visit(Held::Prefetch);
            }
            match wem.parse() {
                Ok(parsed) => visit(Held::Parsed(&parsed)),
                Err(error) => visit(Held::Unreadable(error)),
            }
        }
    }
}

/// Describes the WEM `params` names from the first place that holds all of it.
pub(crate) fn info(
    opened: &Opened,
    params: &WemParams,
    page: Page,
) -> Result<WemInfoResult, ToolError> {
    let (id, places) = candidates(opened, params.wem_id, params.path.as_deref())?;
    page.check(places.len())?;
    let mut first: Option<(LocationShape, WemShape)> = None;
    let mut failure: Option<ToolError> = None;
    for place in places.iter().take(MAX_ATTEMPTS) {
        let size = place.location.len;
        let outcome = read(opened, place, |held| {
            Ok(match held {
                Held::Parsed(wem) => (wem.shape(id, size), true),
                Held::Prefetch => (unreadable(id, size, None), false),
                Held::Unreadable(error) => (unreadable(id, size, Some(&error)), false),
            })
        });
        match outcome {
            Ok((shape, true)) => {
                first = Some((place.location.clone(), shape));
                break;
            }
            Ok((shape, false)) => {
                first.get_or_insert_with(|| (place.location.clone(), shape));
            }
            Err(error) => {
                failure.get_or_insert(error);
            }
        }
    }
    let Some((described, wem)) = first else {
        return Err(failure.unwrap_or(ToolError::WemSelector));
    };
    let total = places.len();
    let mut result = WemInfoResult {
        archive: opened.archive().path().display().to_string(),
        wem_id: id,
        locations_total: total,
        locations: Vec::new(),
        next_cursor: None,
        described,
        wem,
    };
    let (listed, next_cursor) = fill_within(
        page,
        total,
        json_len(&result),
        places
            .into_iter()
            .skip(page.offset())
            .map(|place| Ok(place.location)),
        |location: &LocationShape| format!("location {}", location.path),
    )?;
    result.locations = listed;
    result.next_cursor = next_cursor;
    Ok(result)
}
