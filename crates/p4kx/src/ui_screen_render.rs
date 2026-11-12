// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, Result};
use p4kx_extraction::gfx::{Background, CanvasSize, RasterOptions, Skipped};
use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, Language};
use p4kx_screens::{
    ArchiveScreen, ArchiveScreenError, ArchiveScreenRender, Report, Still, StillFormat,
    StillRequest, Variable, check_variable_count,
};
use serde::Serialize;

use crate::{exit, json};

#[path = "ui_screen_render_origins.rs"]
mod origins;
use self::origins::ScreenEvidence;

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

impl Options<'_> {
    fn request(&self) -> StillRequest<'_> {
        StillRequest {
            screen: self.screen,
            brand: self.brand,
            variables: self.variables,
            raster: RasterOptions {
                size: self.size.unwrap_or(CanvasSize::Stage),
                background: self.background,
            },
            format: if self.svg {
                StillFormat::Svg
            } else {
                StillFormat::Png
            },
        }
    }
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
    #[serde(flatten)]
    evidence: &'a ScreenEvidence<'a>,
    #[serde(rename = "originReceipt")]
    origin_receipt: String,
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

fn draw(
    archive: &Archive,
    options: &Options<'_>,
    cancel: &CancelToken,
) -> Result<ArchiveScreenRender> {
    let entry = DataForgeEntry::read_until(archive, &|| cancel.is_cancelled())?;
    let screen = ArchiveScreen::new(archive, &entry).map_err(cli_error)?;
    let texts = screen.load_texts(Some(options.lang.unwrap_or(Language::English)));
    let mut warnings = Vec::new();
    let rendered = screen.render(&texts, &options.request(), cancel, &mut warnings);
    for warning in &warnings {
        warn(warning);
    }
    rendered.map_err(cli_error)
}

/// Runs `p4kx ui render --screen`.
pub(crate) fn run(archive: &Path, options: &Options<'_>, as_json: bool) -> Result<ExitCode> {
    check_variable_count(options.variables.len())?;
    let archive = crate::key::open(archive)?;
    let cancel = crate::extract::cancel_on_interrupt()?;
    let protection = p4kx_extraction::ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let drawn = draw(&archive, options, &cancel)?;
    let receipt = drawn
        .publish(options.out, &protection, &cancel)
        .map_err(cli_error)?;
    let still = drawn.still();
    let skipped = skipped_json(still.skipped());
    if as_json {
        let name = receipt.file_name().context("missing Screen receipt name")?;
        let evidence = ScreenEvidence::of(still, drawn.origin())?;
        json::print(&RenderJson {
            screen: options.screen,
            target: json::path(options.out),
            format: if options.svg { "svg" } else { "png" },
            width: still.width(),
            height: still.height(),
            report: still.report(),
            skipped: &skipped,
            evidence: &evidence,
            origin_receipt: json::path(Path::new(name)),
        })?;
    } else {
        print_report(options, still, &skipped)?;
    }
    Ok(ExitCode::from(if still.is_complete() {
        exit::SUCCESS
    } else {
        exit::PARTIAL_FAILURE
    }))
}

fn cli_error(error: ArchiveScreenError) -> anyhow::Error {
    match error {
        ArchiveScreenError::DataForge(error) => error.into(),
        ArchiveScreenError::Draw(error) => error.into(),
        ArchiveScreenError::Evidence(error) => error.into(),
        ArchiveScreenError::Publish(error) => error.into(),
        other => other.into(),
    }
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
