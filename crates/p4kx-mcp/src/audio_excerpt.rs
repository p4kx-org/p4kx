// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::Duration;

use p4kx_extraction::AudioError;

use crate::archives::Opened;
use crate::audio_params::ExcerptParams;
use crate::audio_wem::{Candidate, Facts, Held, audio_error, candidates, read};
use crate::audio_wem_shapes::ExcerptResult;
use crate::audio_window::Window;
use crate::error::ToolError;

/// The size cap of the WAV of an excerpt, 1 MiB.
pub(crate) const MAX_EXCERPT_BYTES: usize = 1 << 20;
/// How far into a sound an excerpt may reach, in bytes of decoded audio, 64 MiB.
pub(crate) const MAX_DECODED_BYTES: u64 = 64 << 20;
/// The longest excerpt.
const MAX_EXCERPT: Duration = Duration::from_secs(10);
/// The length of an excerpt when a call gives none.
const DEFAULT_EXCERPT: Duration = Duration::from_secs(5);
/// The most bytes the WAV header of an excerpt takes: RIFF, a 48-byte `fmt ` chunk and `data`.
const MAX_HEAD_LEN: u64 = 12 + 48 + 8;

/// Builds a WAV from the whole `fmt ` chunk (with its header) and the PCM bytes of a window.
pub(crate) fn assemble(format: &[u8], pcm: &[u8]) -> Result<Vec<u8>, AudioError> {
    let riff_len =
        u32::try_from(4 + format.len() + 8 + pcm.len()).map_err(|_| AudioError::WavTooLarge)?;
    let data_len = u32::try_from(pcm.len()).map_err(|_| AudioError::WavTooLarge)?;
    let mut wav = Vec::with_capacity(12 + format.len() + 8 + pcm.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_len.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(format);
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend_from_slice(pcm);
    Ok(wav)
}

/// Returns `seconds` as a duration, or the error that names `name`.
fn seconds(
    name: &'static str,
    seconds: Option<f64>,
    default: Duration,
) -> Result<Duration, ToolError> {
    seconds.map_or(Ok(default), |value| {
        Duration::try_from_secs_f64(value).map_err(|_| ToolError::BadSeconds(name))
    })
}

fn frames_of(time: Duration, rate: u32) -> u64 {
    let frames = time.as_nanos() * u128::from(rate) / 1_000_000_000;
    u64::try_from(frames).unwrap_or(u64::MAX)
}

/// Returns the playing time of `frames` frames at `rate` Hz in seconds.
fn seconds_of(frames: u64, rate: u32) -> f64 {
    let rate = u64::from(rate.max(1));
    let nanos = (frames % rate) * 1_000_000_000 / rate;
    Duration::new(frames / rate, u32::try_from(nanos).unwrap_or(0)).as_secs_f64()
}

/// The window an excerpt asks for.
#[derive(Debug, Clone, Copy)]
struct Ask {
    start: Duration,
    requested: Duration,
}

/// Decodes the WEM `params` names and returns an excerpt of it as a WAV.
pub(crate) fn excerpt(
    opened: &Opened,
    params: &ExcerptParams,
) -> Result<(ExcerptResult, Vec<u8>), ToolError> {
    let ask = Ask {
        start: seconds("start_seconds", params.start_seconds, Duration::ZERO)?,
        requested: seconds("duration_seconds", params.duration_seconds, DEFAULT_EXCERPT)?,
    };
    let (id, places) = candidates(opened, params.wem_id, params.path.as_deref())?;
    let mut prefetched = false;
    let mut failure: Option<ToolError> = None;
    for place in &places {
        match decode(opened, place, id, ask) {
            Ok(Some(done)) => return Ok(done),
            Ok(None) => prefetched = true,
            Err(error) => {
                failure.get_or_insert(error);
            }
        }
    }
    match (failure, prefetched, id) {
        (Some(error), _, _) => Err(error),
        (None, true, Some(id)) => Err(ToolError::PrefetchOnly(id)),
        (None, _, _) => Err(ToolError::WemSelector),
    }
}

/// Where an excerpt starts and how long it may be, in frames.
struct Plan {
    first: u64,
    wanted: u64,
    requested: u64,
    frame_bytes: u64,
}

/// Works out the window of an excerpt from what the WEM header states, refusing one that cannot
/// exist: in an empty sound, behind its end, or too far into the sound to decode.
fn plan(facts: &Facts, ask: Ask) -> Result<Plan, ToolError> {
    let rate = facts.sample_rate;
    let frame_bytes = u64::from(facts.channels).max(1) * 2;
    let first = frames_of(ask.start, rate);
    let room = (MAX_EXCERPT_BYTES as u64).saturating_sub(MAX_HEAD_LEN) / frame_bytes;
    let wanted = frames_of(ask.requested.min(MAX_EXCERPT), rate).min(room);
    if facts.sample_count == Some(0) {
        return Err(ToolError::EmptySound);
    }
    if let Some(count) = facts.sample_count
        && first >= count
    {
        return Err(ToolError::ExcerptStart {
            seconds: seconds_of(count, rate),
        });
    }
    let reach = first
        .saturating_add(wanted)
        .saturating_mul(frame_bytes)
        .saturating_add(MAX_HEAD_LEN);
    if reach > MAX_DECODED_BYTES {
        return Err(ToolError::ExcerptSource {
            limit: MAX_DECODED_BYTES,
        });
    }
    Ok(Plan {
        first,
        wanted,
        requested: frames_of(ask.requested, rate),
        frame_bytes,
    })
}

fn decode(
    opened: &Opened,
    place: &Candidate,
    id: Option<u32>,
    ask: Ask,
) -> Result<Option<(ExcerptResult, Vec<u8>)>, ToolError> {
    read(opened, place, |held| {
        let wem = match held {
            Held::Parsed(wem) => wem,
            Held::Prefetch => return Ok(None),
            Held::Unreadable(error) => return Err(audio_error(&place.location.path, error)),
        };
        let name = &place.location.path;
        let facts = wem.facts();
        let rate = facts.sample_rate;
        let known = facts.sample_count;
        let Plan {
            first,
            wanted,
            requested,
            frame_bytes,
        } = plan(&facts, ask)?;
        let window = Window::new(
            first * frame_bytes,
            (first + wanted) * frame_bytes,
            MAX_DECODED_BYTES - MAX_HEAD_LEN,
            known.is_some(),
        );
        let kept = wem.window(name, window, MAX_DECODED_BYTES)?;
        let got = kept.pcm.len() as u64 / frame_bytes;
        let seen = kept.seen / frame_bytes;
        let total = known.map_or(seen, |count| count.max(seen));
        if total == 0 {
            return Err(ToolError::EmptySound);
        }
        if first >= total || (got == 0 && wanted > 0) {
            return Err(ToolError::ExcerptStart {
                seconds: seconds_of(total, rate),
            });
        }
        let wav = assemble(&kept.format, &kept.pcm).map_err(|error| audio_error(name, error))?;
        if wav.len() > MAX_EXCERPT_BYTES {
            return Err(audio_error(name, AudioError::WavTooLarge));
        }
        let result = ExcerptResult {
            archive: opened.archive().path().display().to_string(),
            wem_id: id,
            source: place.location.clone(),
            codec: facts.codec,
            mime_type: "audio/wav",
            channels: facts.channels,
            sample_rate: rate,
            sound_seconds: seconds_of(total, rate),
            start_seconds: seconds_of(first, rate),
            duration_seconds: seconds_of(got, rate),
            frames: got,
            bytes: wav.len(),
            max_bytes: MAX_EXCERPT_BYTES,
            truncated: got < requested,
        };
        Ok(Some((result, wav)))
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{assemble, frames_of, seconds_of};

    #[test]
    fn an_assembled_wav_keeps_the_format_and_states_its_sizes()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut format = b"fmt ".to_vec();
        format.extend_from_slice(&16_u32.to_le_bytes());
        format.extend_from_slice(&[1, 0, 2, 0, 0xE8, 3, 0, 0, 0xA0, 0xF, 0, 0, 4, 0, 16, 0]);
        let wav = assemble(&format, &[7; 40])?;
        assert_eq!(wav.len(), 12 + 24 + 8 + 40, "header and samples");
        assert_eq!(wav.get(..4), Some(&b"RIFF"[..]), "RIFF");
        let riff = u32::from_le_bytes(wav.get(4..8).ok_or("riff")?.try_into()?);
        assert_eq!(usize::try_from(riff)?, wav.len() - 8, "RIFF size");
        let data = u32::from_le_bytes(wav.get(40..44).ok_or("data")?.try_into()?);
        assert_eq!(data, 40, "data size");
        Ok(())
    }

    #[test]
    fn seconds_and_frames_convert_without_rounding_up() {
        assert_eq!(
            frames_of(Duration::from_millis(1500), 48_000),
            72_000,
            "frames"
        );
        assert!((seconds_of(72_000, 48_000) - 1.5).abs() < 1e-9, "seconds");
    }
}
