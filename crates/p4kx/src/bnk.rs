// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::process::ExitCode;

use anyhow::Result;
use p4kx_extraction::{
    Archive, Bank, BankExtraction, ConversionError, EmbeddedWem, MediaClass, WemFormat,
};
use serde::Serialize;

use crate::cli::{BnkAction, BnkArgs, BnkFormat, BnkSource};
use crate::{bnk_objects, exit, json};

const UNKNOWN: &str = "unknown";

#[derive(Serialize)]
struct ListJson<'a> {
    soundbank: &'a str,
    version: u32,
    bank_id: u32,
    wems: Vec<WemJson>,
}

#[derive(Serialize)]
struct WemJson {
    id: u32,
    size: u64,
    class: &'static str,
    codec: Option<&'static str>,
    codec_name: Option<String>,
    sample_count: Option<u64>,
    duration_seconds: Option<f64>,
    prefetch: bool,
    error: Option<String>,
    #[serde(skip)]
    media: MediaClass,
}

#[derive(Serialize)]
struct ExtractJson<'a> {
    soundbank: &'a str,
    target: String,
    format: &'static str,
    extracted: Vec<WrittenJson>,
    failed: Vec<FailedJson>,
}

#[derive(Serialize)]
struct WrittenJson {
    id: u32,
    file: String,
    size: u64,
    prefetch: bool,
}

#[derive(Serialize)]
struct FailedJson {
    id: u32,
    error: String,
}

/// Runs `p4kx bnk list` or `p4kx bnk extract`.
pub(crate) fn run(args: &BnkArgs, as_json: bool) -> Result<ExitCode> {
    match &args.action {
        BnkAction::List(source) => {
            list(&open(source)?, as_json)?;
            Ok(ExitCode::from(exit::SUCCESS))
        }
        BnkAction::Objects(objects) => bnk_objects::run(objects, as_json),
        BnkAction::Extract {
            source,
            out,
            format,
        } => extract(&open(source)?, out, *format, as_json),
    }
}

pub(crate) fn open(source: &BnkSource) -> Result<Bank> {
    open_with(source, Bank::from_archive, Bank::from_file)
}

pub(crate) fn open_observed(source: &BnkSource) -> Result<Bank> {
    open_with(
        source,
        Bank::from_archive_observed,
        Bank::from_file_observed,
    )
}

fn open_with(
    source: &BnkSource,
    archive: fn(&Archive, &str) -> Result<Bank, ConversionError>,
    file: fn(&Path) -> Result<Bank, ConversionError>,
) -> Result<Bank> {
    Ok(match &source.entry {
        Some(entry) => archive(&crate::key::open(&source.path)?, entry)?,
        None => file(&source.path)?,
    })
}

fn describe(wem: &EmbeddedWem<'_>) -> WemJson {
    let class = wem.class();
    let mut json = WemJson {
        id: wem.id(),
        size: wem.bytes().len() as u64,
        class: class.name(),
        codec: None,
        codec_name: None,
        sample_count: None,
        duration_seconds: None,
        prefetch: wem.is_prefetch(),
        error: None,
        media: class,
    };
    if json.prefetch || class.is_non_sound() {
        return json;
    }
    match wem.parse() {
        Ok(parsed) => {
            json.codec = Some(parsed.codec().id());
            json.codec_name = Some(parsed.codec().to_string());
            json.sample_count = parsed.sample_count();
            json.duration_seconds = parsed.duration().map(|duration| duration.as_secs_f64());
        }
        Err(error) => json.error = Some(error.to_string()),
    }
    json
}

fn list(bank: &Bank, as_json: bool) -> Result<()> {
    let soundbank = bank.soundbank();
    let wems: Vec<WemJson> = soundbank.wems().map(|wem| describe(&wem)).collect();
    if as_json {
        json::print(&ListJson {
            soundbank: bank.name(),
            version: soundbank.header().version,
            bank_id: soundbank.header().id,
            wems,
        })?;
        return Ok(());
    }
    let mut out = BufWriter::new(io::stdout().lock());
    writeln!(out, "Soundbank:   {}", bank.name())?;
    writeln!(out, "Version:     {}", soundbank.header().version)?;
    writeln!(out, "Bank ID:     {}", soundbank.header().id)?;
    writeln!(out, "WEMs:        {}", wems.len())?;
    if !wems.is_empty() {
        writeln!(out)?;
        writeln!(
            out,
            "{:>10}  {:<13}  {:<16}  {:>10}",
            "ID", "Class", "Codec", "Duration"
        )?;
    }
    for wem in &wems {
        let codec = match &wem.codec_name {
            Some(name) => name,
            None if wem.prefetch => "prefetch",
            None if wem.media.is_non_sound() => "-",
            None => "invalid",
        };
        let duration = wem
            .duration_seconds
            .map_or_else(|| UNKNOWN.to_owned(), |seconds| format!("{seconds:.3} s"));
        write!(
            out,
            "{:>10}  {:<13}  {codec:<16}  {duration:>10}",
            wem.id, wem.media
        )?;
        if let Some(error) = wem.error.as_ref() {
            write!(out, "  {error}")?;
        }
        writeln!(out)?;
    }
    out.flush()?;
    Ok(())
}

fn extract(bank: &Bank, out: &Path, format: BnkFormat, as_json: bool) -> Result<ExitCode> {
    let format = match format {
        BnkFormat::Wem => WemFormat::Wem,
        BnkFormat::Wav => WemFormat::Wav,
        BnkFormat::Ogg => WemFormat::Ogg,
    };
    let extraction = bank.extract_wems(out, format)?;
    let total = extraction.written.len() + extraction.failures.len();
    if as_json {
        json::print(&extract_json(bank, out, format, &extraction))?;
    } else {
        let mut stderr = io::stderr().lock();
        for failure in &extraction.failures {
            writeln!(stderr, "error: {}", chain(&failure.error))?;
        }
        let prefetched = extraction
            .written
            .iter()
            .filter(|written| written.prefetch)
            .count();
        let mut stdout = io::stdout().lock();
        write!(
            stdout,
            "extracted {} of {total} WEMs from {} into {}",
            extraction.written.len(),
            bank.name(),
            out.display()
        )?;
        if prefetched > 0 {
            write!(
                stdout,
                "; {prefetched} of them only the prefetched start of a streamed WEM"
            )?;
        }
        writeln!(stdout)?;
    }
    Ok(ExitCode::from(if extraction.failures.is_empty() {
        exit::SUCCESS
    } else {
        exit::PARTIAL_FAILURE
    }))
}

fn extract_json<'a>(
    bank: &'a Bank,
    out: &Path,
    format: WemFormat,
    extraction: &BankExtraction,
) -> ExtractJson<'a> {
    ExtractJson {
        soundbank: bank.name(),
        target: json::path(out),
        format: format.extension(),
        extracted: extraction
            .written
            .iter()
            .map(|written| WrittenJson {
                id: written.id,
                file: json::path(&written.path),
                size: written.size,
                prefetch: written.prefetch,
            })
            .collect(),
        failed: extraction
            .failures
            .iter()
            .map(|failure| FailedJson {
                id: failure.id,
                error: chain(&failure.error),
            })
            .collect(),
    }
}

fn chain(error: &(dyn std::error::Error + 'static)) -> String {
    let mut text = String::new();
    let mut current = Some(error);
    while let Some(error) = current {
        if !text.is_empty() {
            text.push_str(": ");
        }
        text.push_str(&error.to_string());
        current = error.source();
    }
    text
}
