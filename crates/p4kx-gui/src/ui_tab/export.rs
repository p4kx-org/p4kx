// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};
use std::sync::Arc;

use p4kx_extraction::{
    ArchiveWriteProtection, CancelToken, ScreenTexts, WriteProtectionError, is_same_file,
};
use p4kx_screens::{ArchiveScreen, ArchiveScreenError, ScreenError, StillFormat, StillRequest};

use super::job::{Job, Poll};
use super::render::{Fail, RenderKey, UiSource, checkpoint, open_movie, reason};
use super::report::UiReport;
use super::settings::UiFormat;
use crate::EntryIndex;
use crate::UiText;

/// What an export wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiExportSummary {
    /// The file written.
    pub file: PathBuf,
    /// The width of the picture in pixels.
    pub width: u32,
    /// The height of the picture in pixels.
    pub height: u32,
    /// How many things the render left out, as the report of the picture lists them.
    pub left_out: usize,
}

/// Where an export stands.
#[derive(Debug, Clone, Copy)]
pub enum UiExportStatus<'a> {
    /// The picture is drawn and written off the UI thread.
    Running,
    /// The file was written.
    Done(&'a UiExportSummary),
    /// The file was written, but the Archive changed on disk meanwhile, so it may mix old and new
    /// data.
    DoneArchiveChanged(&'a UiExportSummary),
    /// The file is the open Archive, which is never overwritten.
    IsArchive,
    /// The Archive changed on disk before the file was written, so nothing was written.
    ArchiveChanged,
    /// The output violates a protection or identity boundary.
    Refused(&'a WriteProtectionError),
    /// Writing failed for the reason given, or stopped without a result.
    Failed(Option<&'a str>),
}

enum Outcome {
    Done(UiExportSummary),
    DoneArchiveChanged(UiExportSummary),
    IsArchive,
    ArchiveChanged,
    Stopped,
    Failed(String),
    Refused(WriteProtectionError),
}

enum State {
    Running(Job<Outcome>),
    Done(UiExportSummary),
    DoneArchiveChanged(UiExportSummary),
    IsArchive,
    ArchiveChanged,
    Refused(WriteProtectionError),
    Failed(Option<String>),
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Running(_) => "Running",
            Self::Done(_) => "Done",
            Self::DoneArchiveChanged(_) => "DoneArchiveChanged",
            Self::IsArchive => "IsArchive",
            Self::ArchiveChanged => "ArchiveChanged",
            Self::Refused(_) => "Refused",
            Self::Failed(_) => "Failed",
        })
    }
}

/// The running or finished export of the picture of the tab into a file.
#[derive(Debug)]
pub struct UiExport {
    file: PathBuf,
    state: State,
}

impl UiExport {
    /// Returns the file the export writes.
    #[must_use]
    pub fn file(&self) -> &Path {
        &self.file
    }

    /// Returns where the export stands.
    #[must_use]
    pub fn status(&self) -> UiExportStatus<'_> {
        match &self.state {
            State::Running(_) => UiExportStatus::Running,
            State::Done(summary) => UiExportStatus::Done(summary),
            State::DoneArchiveChanged(summary) => UiExportStatus::DoneArchiveChanged(summary),
            State::IsArchive => UiExportStatus::IsArchive,
            State::ArchiveChanged => UiExportStatus::ArchiveChanged,
            State::Refused(installation) => UiExportStatus::Refused(installation),
            State::Failed(reason) => UiExportStatus::Failed(reason.as_deref()),
        }
    }

    /// Returns whether the export still runs.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        matches!(self.state, State::Running(_))
    }

    /// Takes the outcome once the export ended; returns whether it did.
    pub fn poll(&mut self) -> bool {
        let State::Running(job) = &self.state else {
            return false;
        };
        self.state = match job.poll() {
            Poll::Pending => return false,
            Poll::Ready(Outcome::Done(summary)) => State::Done(summary),
            Poll::Ready(Outcome::DoneArchiveChanged(summary)) => State::DoneArchiveChanged(summary),
            Poll::Ready(Outcome::IsArchive) => State::IsArchive,
            Poll::Ready(Outcome::ArchiveChanged) => State::ArchiveChanged,
            Poll::Ready(Outcome::Failed(reason)) => State::Failed(Some(reason)),
            Poll::Ready(Outcome::Refused(error)) => State::Refused(error),
            Poll::Ready(Outcome::Stopped) | Poll::Lost => State::Failed(None),
        };
        true
    }
}

/// What an export draws and writes.
pub(super) struct Request {
    pub(super) key: RenderKey,
    pub(super) format: UiFormat,
    pub(super) file: PathBuf,
    pub(super) texts: Option<Arc<ScreenTexts>>,
}

/// Returns an export refused by an output-protection boundary.
pub(super) const fn refused(file: PathBuf, refused: WriteProtectionError) -> UiExport {
    UiExport {
        file,
        state: State::Refused(refused),
    }
}

pub(super) fn start(
    index: &Arc<EntryIndex>,
    request: Request,
    wake: impl Fn() + Send + 'static,
) -> UiExport {
    let file = request.file.clone();
    let index = Arc::clone(index);
    let spawned = Job::spawn_stoppable("ui-export", wake, move |cancel| {
        run(&index, &request, cancel)
    });
    let state = match spawned {
        Ok(job) => State::Running(job),
        Err(reason) => State::Failed(Some(reason)),
    };
    UiExport { file, state }
}

fn run(index: &EntryIndex, request: &Request, cancel: &CancelToken) -> Outcome {
    if is_same_file(&request.file, index.path()) {
        return Outcome::IsArchive;
    }
    if index.watch().check().is_err() {
        return Outcome::ArchiveChanged;
    }
    let written = write(index, request, cancel);
    match written {
        Ok(summary) if index.watch().check().is_err() => Outcome::DoneArchiveChanged(summary),
        Ok(summary) => Outcome::Done(summary),
        Err(Fail::ArchiveChanged) => Outcome::ArchiveChanged,
        Err(Fail::Stopped) => Outcome::Stopped,
        Err(Fail::Reason(reason)) => Outcome::Failed(reason),
        Err(Fail::Protected(error)) => Outcome::Refused(error),
    }
}

fn counted(report: &UiReport) -> usize {
    report
        .lines()
        .iter()
        .filter(|line| line.label != UiText::ReportWarning)
        .count()
}

fn write(
    index: &EntryIndex,
    request: &Request,
    cancel: &CancelToken,
) -> Result<UiExportSummary, Fail> {
    let (key, file) = (&request.key, &request.file);
    checkpoint(cancel)?;
    match &key.source {
        UiSource::Movie(path) => {
            let (movie, warnings) = open_movie(index, path, key.language)?;
            checkpoint(cancel)?;
            let options = key.raster();
            let (width, height, report) = match request.format {
                UiFormat::Png => {
                    let done = movie
                        .render_to_file(&options, file)
                        .map_err(|e| reason(&e))?;
                    let report = UiReport::of_movie(
                        &done.skipped,
                        &done.bitmap_failures,
                        &done.tag_failures,
                        &done.imports,
                    );
                    (done.image.width(), done.image.height(), report)
                }
                UiFormat::Svg => {
                    let done = movie
                        .render_svg_to_file(&options, file)
                        .map_err(|e| reason(&e))?;
                    let report = UiReport::of_movie(
                        &done.skipped,
                        &done.bitmap_failures,
                        &done.tag_failures,
                        &done.imports,
                    );
                    (done.width, done.height, report)
                }
            };
            Ok(UiExportSummary {
                file: file.clone(),
                width,
                height,
                left_out: counted(&report.warned(&warnings)),
            })
        }
        UiSource::Screen { canvas, brand } => {
            write_screen(index, (request, cancel), (canvas, brand.as_deref()))
        }
    }
}

fn write_screen(
    index: &EntryIndex,
    (request, cancel): (&Request, &CancelToken),
    (canvas, brand): (&str, Option<&str>),
) -> Result<UiExportSummary, Fail> {
    let (key, file) = (&request.key, &request.file);
    let source = index.forge().read(index, || {})?;
    checkpoint(cancel)?;
    let archive = index.entries_archive().map_err(|error| reason(&error))?;
    let screen = ArchiveScreen::new(archive, &source.entry).map_err(|error| reason(&error))?;
    checkpoint(cancel)?;
    let asked = StillRequest {
        screen: canvas,
        brand,
        variables: &key.variables,
        raster: key.raster(),
        format: match request.format {
            UiFormat::Png => StillFormat::Png,
            UiFormat::Svg => StillFormat::Svg,
        },
    };
    let draw = |texts: &ScreenTexts| {
        let mut warnings = Vec::new();
        screen
            .render(texts, &asked, cancel, &mut warnings)
            .map(|drawn| (drawn, warnings))
    };
    let (drawn, warnings) = request
        .texts
        .as_deref()
        .map_or_else(|| draw(&screen.load_texts(key.language)), draw)
        .map_err(|error| match error {
            ArchiveScreenError::Draw(ScreenError::Stopped) => Fail::Stopped,
            other => reason(&other),
        })?;
    checkpoint(cancel)?;
    if index.watch().check().is_err() {
        return Err(Fail::ArchiveChanged);
    }
    let protection =
        ArchiveWriteProtection::new(index.path(), cancel.flag()).map_err(|error| reason(&error))?;
    drawn
        .publish(file, &protection, cancel)
        .map_err(|error| reason(&error))?;
    let still = drawn.still();
    Ok(UiExportSummary {
        file: file.clone(),
        width: still.width(),
        height: still.height(),
        left_out: counted(&UiReport::of_still(still, &warnings)),
    })
}
