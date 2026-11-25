// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use p4kx_extraction::{Codebooks, Codec, Sound};
use serde::Serialize;

use crate::cli::{AudioAction, AudioArgs, AudioSource};
use crate::json;

#[path = "audio_info_origins.rs"]
mod info_origins;

const UNKNOWN: &str = "unknown";
const NONE: &str = "none";

#[derive(Serialize)]
struct LoopJson {
    start: u64,
    end: u64,
    frames: u64,
}

#[derive(Serialize)]
struct InfoJson<'a> {
    sound: &'a str,
    codec: &'static str,
    codec_name: String,
    format_tag: u16,
    channels: u16,
    sample_rate: u32,
    sample_count: Option<u64>,
    duration_seconds: Option<f64>,
    #[serde(rename = "loop")]
    loop_points: Option<LoopJson>,
}

#[derive(Serialize)]
struct WavJson<'a> {
    sound: &'a str,
    output: String,
    #[serde(rename = "originReceipt", skip_serializing_if = "Option::is_none")]
    origin_receipt: Option<String>,
    channels: u16,
    sample_rate: u32,
    sample_count: u64,
    size: u64,
    #[serde(rename = "loop")]
    loop_points: Option<LoopJson>,
}

#[derive(Serialize)]
struct OggJson<'a> {
    sound: &'a str,
    output: String,
    codec: &'static str,
    channels: u16,
    sample_rate: u32,
    sample_count: u64,
    duration_seconds: f64,
    codebooks: Option<&'static str>,
    size: u64,
    #[serde(rename = "loop")]
    loop_points: Option<LoopJson>,
}

/// Runs `p4kx audio info`, `p4kx audio wav` or `p4kx audio ogg`.
pub(crate) fn run(args: &AudioArgs, as_json: bool) -> Result<()> {
    match &args.action {
        AudioAction::Info(source) => info(&open(source, as_json)?, as_json),
        AudioAction::Wav { source, out } => {
            let sound = if as_json || source.entry.is_some() {
                open(source, true)?
            } else {
                Sound::from_file_observed_for_wav(&source.path)?
            };
            wav(&sound, out, as_json)
        }
        AudioAction::Ogg { source, out } => ogg(&open(source, as_json)?, out, as_json),
    }
}

fn open(source: &AudioSource, observe: bool) -> Result<Sound> {
    Ok(match (&source.entry, observe) {
        (Some(entry), true) => {
            Sound::from_archive_observed(&crate::key::open(&source.path)?, entry)?
        }
        (Some(entry), false) => Sound::from_archive(&crate::key::open(&source.path)?, entry)?,
        (None, true) => Sound::from_file_observed(&source.path)?,
        (None, false) => Sound::from_file(&source.path)?,
    })
}

fn info(sound: &Sound, as_json: bool) -> Result<()> {
    let wem = sound.wem();
    let duration = wem.duration();
    let info = InfoJson {
        sound: sound.name(),
        codec: wem.codec().id(),
        codec_name: wem.codec().to_string(),
        format_tag: wem.format_tag(),
        channels: wem.channels(),
        sample_rate: wem.sample_rate(),
        sample_count: wem.sample_count(),
        duration_seconds: duration.map(|duration| duration.as_secs_f64()),
        loop_points: wem
            .loop_points()
            .map(|points| loop_json(points.start, points.end)),
    };
    if as_json {
        json::print(&info_origins::report(sound, info)?)?;
        return Ok(());
    }
    let lines = [
        ("Sound", info.sound.to_owned()),
        ("Codec", info.codec_name),
        ("Channels", info.channels.to_string()),
        ("Sample rate", format!("{} Hz", info.sample_rate)),
        (
            "Duration",
            duration.map_or_else(|| UNKNOWN.to_owned(), format_duration),
        ),
        (
            "Loop",
            info.loop_points.as_ref().map_or_else(
                || NONE.to_owned(),
                |points| {
                    format!(
                        "frames {} to {}, {} frames",
                        points.start, points.end, points.frames
                    )
                },
            ),
        ),
    ];
    let mut out = io::stdout().lock();
    for (label, value) in lines {
        writeln!(out, "{:<12} {value}", format!("{label}:"))?;
    }
    Ok(())
}

const fn loop_json(start: u64, end: u64) -> LoopJson {
    LoopJson {
        start,
        end,
        frames: end.saturating_sub(start),
    }
}

fn format_duration(duration: Duration) -> String {
    let millis = duration.as_millis();
    format!("{}.{:03} s", millis / 1000, millis % 1000)
}

fn wav(sound: &Sound, out: &Path, as_json: bool) -> Result<()> {
    let observed = sound
        .wem()
        .supports_linear_wav()
        .then(|| sound.write_linear_wav_with_receipt(out))
        .transpose()?;
    let report = match &observed {
        Some(receipt) => receipt.report,
        None => sound.write_wav_to_file(out)?,
    };
    let companion = observed.as_ref().map(|receipt| &receipt.companion);
    if as_json {
        let origin_receipt = if let Some(companion) = companion {
            let basename = companion
                .file_name()
                .ok_or_else(|| io::Error::other("missing origin companion basename"))?;
            Some(json::path(Path::new(basename)))
        } else {
            None
        };
        let summary = WavJson {
            sound: sound.name(),
            output: json::path(out),
            origin_receipt,
            channels: report.channels,
            sample_rate: report.sample_rate,
            sample_count: report.sample_count,
            size: report.size,
            loop_points: report
                .loop_points
                .map(|points| loop_json(points.start, points.end)),
        };
        if let Some(receipt) = &observed {
            json::print(&info_origins::wav_report(receipt, summary)?)?;
        } else {
            json::print(&info_origins::conversion_report(sound, summary)?)?;
        }
    } else {
        writeln!(
            io::stdout().lock(),
            "wrote {} as WAV to {}, {} bytes",
            sound.name(),
            out.display(),
            report.size
        )?;
        if let Some(companion) = companion {
            writeln!(
                io::stdout().lock(),
                "source origins: {}",
                companion.display()
            )?;
        }
    }
    Ok(())
}

fn ogg(sound: &Sound, out: &Path, as_json: bool) -> Result<()> {
    let report = sound.write_ogg_to_file(out)?;
    let duration = report.duration();
    if as_json {
        let summary = OggJson {
            sound: sound.name(),
            output: json::path(out),
            codec: report.codec.id(),
            channels: report.channels,
            sample_rate: report.sample_rate,
            sample_count: report.sample_count,
            duration_seconds: duration.as_secs_f64(),
            codebooks: report.codebooks.map(Codebooks::id),
            size: report.size,
            loop_points: report
                .loop_points
                .map(|points| loop_json(points.start, points.end)),
        };
        json::print(&info_origins::conversion_report(sound, summary)?)?;
    } else {
        writeln!(
            io::stdout().lock(),
            "wrote {} as Ogg {} to {}, {}, {} bytes",
            sound.name(),
            if report.codec == Codec::Opus {
                "Opus"
            } else {
                "Vorbis"
            },
            out.display(),
            format_duration(duration),
            report.size
        )?;
    }
    Ok(())
}
