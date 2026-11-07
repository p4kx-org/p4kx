// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use anyhow::Result;
use p4kx_extraction::gfx::{Background, CanvasSize, Localise, RasterOptions, Skipped};
use p4kx_extraction::{
    Archive, ArchiveImages, DataForgeEntry, Language, ScreenTexts, decode_line_breaks,
    write_replacing_protected,
};
use p4kx_screens::{
    ImageSources, MAX_SVG_BYTES, Report, ScreenComposer, Still, StillFormat, StillRequest,
    TextSources, Variable, check_variable_count,
};
use serde::Serialize;

use crate::{exit, json};

pub(crate) struct Options<'a> {
    pub(crate) screen: &'a str,
    pub(crate) brand: Option<&'a str>,
    pub(crate) variables: &'a [Variable],
    pub(crate) lang: Option<Language>,
    pub(crate) out: &'a Path,
    pub(crate) size: Option<CanvasSize>,
    pub(crate) background: Background,
    pub(crate) svg: bool,
}

#[derive(Serialize)]
struct RenderJson<'a> {
    screen: &'a str,
    target: String,
    format: &'static str,
    width: u32,
    height: u32,
    #[serde(flatten)]
    report: &'a Report,
    skipped: &'a [SkippedJson],
}

#[derive(Serialize)]
struct SkippedJson {
    reason: &'static str,
    message: String,
}

fn skipped_json(skipped: &[Skipped]) -> Vec<SkippedJson> {
    skipped
        .iter()
        .map(|skipped| SkippedJson {
            reason: skipped.reason.code(),
            message: skipped.reason.to_string(),
        })
        .collect()
}

fn draw(archive: &Archive, options: &Options<'_>) -> Result<Still> {
    let entry = DataForgeEntry::read(archive)?;
    let forge = entry.open()?;
    let texts = ScreenTexts::load(
        archive,
        &ScreenComposer::new(&forge).font_file(),
        Some(options.lang.unwrap_or(Language::English)),
    );
    let mut warnings = texts.warnings().to_vec();
    let ini = texts.parsed(&mut warnings);
    for warning in &warnings {
        warn(warning);
    }
    let lookup = |key: &str| {
        ini.as_ref()?
            .resolve(key)
            .map(|text| decode_line_breaks(text).into_owned())
    };
    let images = ArchiveImages::new(archive);
    let read_texture = |path: &str, side: u32, pixels: u64| {
        images
            .texture(path, side, pixels)
            .map_err(|error| describe(&error))
    };
    let read_vector = |path: &str| {
        images
            .vector(path, MAX_SVG_BYTES as u64)
            .map_err(|error| describe(&error))
    };
    let request = StillRequest {
        screen: options.screen,
        brand: options.brand,
        variables: options.variables,
        raster: RasterOptions {
            size: options.size.unwrap_or(CanvasSize::Stage),
            background: options.background,
        },
        format: if options.svg {
            StillFormat::Svg
        } else {
            StillFormat::Png
        },
    };
    let text_sources = texts.fonts().map(|fonts| TextSources {
        fonts,
        localise: ini.as_ref().map(|_| &lookup as Localise<'_>),
    });
    let image_sources = ImageSources {
        texture: &read_texture,
        vector: &read_vector,
    };
    Ok(Still::render(
        &forge,
        text_sources,
        Some(image_sources),
        &request,
    )?)
}

/// Runs `p4kx ui render --screen`.
pub(crate) fn run(archive: &Path, options: &Options<'_>, as_json: bool) -> Result<ExitCode> {
    check_variable_count(options.variables.len())?;
    let archive = crate::key::open(archive)?;
    let cancel = crate::extract::cancel_on_interrupt()?;
    let protection = p4kx_extraction::ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let still = draw(&archive, options)?;
    let bytes = still.encode()?;
    write_replacing_protected(options.out, Some(&protection), Some(&cancel), |writer| {
        writer
            .write_all(&bytes)
            .map_err(|source| p4kx_extraction::ConversionError::Write {
                path: options.out.to_path_buf(),
                source,
            })
    })?;
    let skipped = skipped_json(still.skipped());
    if as_json {
        json::print(&RenderJson {
            screen: options.screen,
            target: json::path(options.out),
            format: if options.svg { "svg" } else { "png" },
            width: still.width(),
            height: still.height(),
            report: still.report(),
            skipped: &skipped,
        })?;
    } else {
        print_report(options, &still, &skipped)?;
    }
    Ok(ExitCode::from(if still.is_complete() {
        exit::SUCCESS
    } else {
        exit::PARTIAL_FAILURE
    }))
}

fn describe(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

fn warn(error: &impl std::fmt::Display) {
    drop(writeln!(
        io::stderr().lock(),
        "warning: texts are drawn without it: {error}"
    ));
}

fn print_report(options: &Options<'_>, still: &Still, skipped: &[SkippedJson]) -> io::Result<()> {
    let report = still.report();
    let (width, height) = (still.width(), still.height());
    let mut stderr = io::stderr().lock();
    for skipped in skipped {
        writeln!(stderr, "skipped by the renderer: {}", skipped.message)?;
    }
    for entry in &report.unsupported {
        writeln!(
            stderr,
            "left out: {} x{}: {} ({})",
            entry.element,
            entry.count,
            entry.reason,
            entry.examples.join(", ")
        )?;
    }
    for name in report.unknown_variables() {
        writeln!(
            stderr,
            "unknown game variable: {name} (no operation of the screen reads it)"
        )?;
    }
    for canvas in &report.missing_canvases {
        writeln!(stderr, "canvas not drawn: {canvas}")?;
    }
    for texture in &report.missing_textures {
        writeln!(stderr, "texture not in the Archive: {texture}")?;
    }
    if report.truncated {
        writeln!(
            stderr,
            "stopped early: a limit of the composition was reached"
        )?;
    }
    drop(stderr);
    let mut stdout = io::stdout().lock();
    if !report.variables.is_empty() {
        let forced: Vec<String> = report
            .variables
            .iter()
            .map(|variable| format!("{}={}", variable.name, variable.value))
            .collect();
        writeln!(stdout, "game variables in force: {}", forced.join(", "))?;
    }
    writeln!(
        stdout,
        "rendered {} into {}, {width} x {height} px; {} widgets in {} canvases, {} shapes, {} texts drawn and {} left out, {} images drawn and {} left out, {} kinds of element left out",
        options.screen,
        options.out.display(),
        report.widgets,
        report.canvases,
        report.shapes,
        report.texts_drawn,
        report.texts_left_out,
        report.images_drawn,
        report.images_left_out,
        report.unsupported.len()
    )
}
