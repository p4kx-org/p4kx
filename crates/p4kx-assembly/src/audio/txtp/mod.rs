// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Writes vgmstream TXTP text files that play the exported sounds of a Wwise event the way its
//! `HIRC` objects combine them. Only text is written; no vgmstream code is used.
//!
//! The syntax is that of vgmstream's `doc/TXTP.md`: one file line per sound, with the loop of
//! its WEM as `#I start end`, and one `group` line after the children of every object that plays
//! several: `-L<n>` layers, `-S<n>` segments and `-R<n>>1` for a choice at run time, which the
//! file settles on the first child. Names in a TXTP are file names of the export folder, so they
//! follow the portable rule of ADR-0248.

mod approximation;
mod diagnostic;
mod loop_origin;
mod origins;
mod output;
mod path_origin;
mod render;
mod selection;
mod tree;
mod wem_origin;

pub use self::approximation::TxtpApproximation;
pub use self::origins::TxtpOrigins;

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use p4kx_audio::Loop;
use p4kx_extraction::{Archive, ArchiveWatch, ArchiveWriteProtection, CancelToken, ConflictPolicy};

use self::origins::OriginInputs;
use self::render::{RenderedTxtp, SoundFile, comment_text, render};
use super::export::name::{StemSet, portable_stem};
use super::export_preflight;
use super::{
    ExportLoopOrigins, ExportReport, ExportState, ExportedSound, SoundVariant, TriggerSounds,
};

/// The extension of a TXTP file, without the dot.
const EXTENSION: &str = "txtp";

/// The stem of a TXTP whose event name is too long for a file name.
const FALLBACK_STEM: &str = "event";

/// A TXTP file of an export.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TxtpFile {
    /// The audio trigger the Wwise event belongs to.
    pub trigger: String,
    /// The name of the Wwise event the file plays.
    pub event: String,
    /// The file.
    pub path: PathBuf,
    /// The size of the file in bytes.
    pub size: u64,
    /// Whether the file was written or kept.
    pub state: ExportState,
    /// How many broken links the walk met in the event; above 0 the file is incomplete, as its
    /// header says, because the objects behind a broken link are not played.
    pub breaks: usize,
    /// Approximation codes for written or byte-verified content, or `None` when unchecked.
    pub approximations: Option<Vec<TxtpApproximation>>,
    /// Loop frames of the rendered sound commands, in file-line order, when any loop exists.
    pub command_loops: Option<Vec<Option<Loop>>>,
    /// Field-addressed origin of the diagnosed and rendered values when reported.
    pub origins: Option<TxtpOrigins>,
    /// Companion path for a file committed by this export.
    pub origin_receipt: Option<PathBuf>,
}

/// A Wwise event that got no TXTP.
///
/// The reason is one of: a sound has no whole file, or the file could not be written, for
/// instance because its path is too long.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxtpSkip {
    /// The audio trigger the Wwise event belongs to.
    pub trigger: String,
    /// The name of the Wwise event.
    pub event: String,
    /// Why no file was written.
    pub reason: String,
}

/// What a TXTP export did: the files in trigger and event order, and the events without one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TxtpReport {
    /// The files written or kept.
    pub files: Vec<TxtpFile>,
    /// The events that got no file.
    pub skipped: Vec<TxtpSkip>,
}

impl TxtpReport {
    /// Returns how many files were written.
    #[must_use]
    pub fn written_count(&self) -> usize {
        self.files
            .iter()
            .filter(|file| file.state == ExportState::Written)
            .count()
    }

    /// Returns how many existing files were kept.
    #[must_use]
    pub fn kept_count(&self) -> usize {
        self.files.len() - self.written_count()
    }
}

/// Writes one TXTP per Wwise event of `triggers` that leads to sounds, into `folder` as
/// `<event>.txtp`, naming the files `exported` holds.
///
/// A name is escaped to pass the portable file-name rule of ADR-0248 (`%` and two hex digits for
/// each character it forbids, and for `#` and a leading space, which a TXTP line cannot hold), an
/// empty name becomes `%`, and a name that is too long becomes `event`; a name already taken
/// under the case fold of that rule gets `_2`, `_3`, … appended in event order. An event whose
/// sounds are not all in `exported` as a whole file, because a source has no file or only its prefetched start was written, is listed
/// in [`TxtpReport::skipped`] with the reason. Objects the walk does not tell apart are combined
/// as follows: an Event's actions, blend containers, music segments and music tracks as layers;
/// sequence containers and music playlists as segments; random and switch containers and music
/// switches as a choice. The report identifies observed timing, conditions, authored
/// container loops, Blend curves and `MusicTrack` automation omitted by the generated commands.
///
/// An event with broken links is written with a header comment that says it is incomplete, and
/// [`TxtpFile::breaks`] counts them. A file that cannot be written, e.g. because its path is too
/// long, is listed in [`TxtpReport::skipped`] with the error; the other events are still written.
/// A traversal limit skips the export before writing and reports the blocking event.
#[must_use]
pub fn write_txtp(
    triggers: &[&TriggerSounds<'_>],
    exported: &ExportReport,
    folder: &Path,
    policy: ConflictPolicy,
) -> TxtpReport {
    if let Some(cut) = export_preflight::traversal_cut(triggers) {
        return TxtpReport {
            files: Vec::new(),
            skipped: vec![TxtpSkip {
                trigger: cut.trigger.to_owned(),
                event: comment_text(cut.event),
                reason: format!("audio resolution incomplete ({})", cut.reason),
            }],
        };
    }
    write_files(
        triggers,
        exported,
        folder,
        &Destination {
            policy,
            origins: None,
        },
    )
}

/// Writes TXTP and binds its field-origin companion to each newly published file.
///
/// Pass triggers resolved with physical path sources from the same archive.
///
/// # Errors
/// Returns an error when resolution was truncated, the archive changed or write protection fails.
#[must_use = "the report carries the write outcomes"]
pub fn write_txtp_with_origins(
    archive: &Archive,
    triggers: &[&TriggerSounds<'_>],
    exported: &ExportReport,
    folder: &Path,
    policy: ConflictPolicy,
    cancel: &CancelToken,
) -> io::Result<TxtpReport> {
    write_with_evidence(archive, triggers, exported, folder, policy, cancel, None)
}

/// Writes TXTP origins using loop evidence from the same sound export.
///
/// # Errors
/// Returns the same output errors as [`write_txtp_with_origins`].
pub fn write_txtp_with_loop_origins(
    archive: &Archive,
    triggers: &[&TriggerSounds<'_>],
    exported: &ExportReport,
    folder: &Path,
    policy: ConflictPolicy,
    cancel: &CancelToken,
    loop_origins: &ExportLoopOrigins,
) -> io::Result<TxtpReport> {
    write_with_evidence(
        archive,
        triggers,
        exported,
        folder,
        policy,
        cancel,
        Some(loop_origins),
    )
}

fn write_with_evidence(
    archive: &Archive,
    triggers: &[&TriggerSounds<'_>],
    exported: &ExportReport,
    folder: &Path,
    policy: ConflictPolicy,
    cancel: &CancelToken,
    loop_origins: Option<&ExportLoopOrigins>,
) -> io::Result<TxtpReport> {
    if let Some(cut) = export_preflight::traversal_cut(triggers) {
        return Err(io::Error::other(cut.into_error()));
    }
    ArchiveWatch::new(archive)
        .check()
        .map_err(io::Error::other)?;
    let protection =
        ArchiveWriteProtection::new(archive.path(), cancel.flag()).map_err(io::Error::other)?;
    let origins = output::OriginContext {
        archive,
        protection: &protection,
        cancel,
        loop_origins,
    };
    Ok(write_files(
        triggers,
        exported,
        folder,
        &Destination {
            policy,
            origins: Some(origins),
        },
    ))
}

struct Destination<'a> {
    policy: ConflictPolicy,
    origins: Option<output::OriginContext<'a>>,
}

fn write_files(
    triggers: &[&TriggerSounds<'_>],
    exported: &ExportReport,
    folder: &Path,
    destination: &Destination<'_>,
) -> TxtpReport {
    let sounds: HashMap<(&str, u32), &ExportedSound> = exported
        .sounds
        .iter()
        .map(|sound| ((sound.trigger.as_str(), sound.wem_id), sound))
        .collect();
    let loop_origins = destination
        .origins
        .as_ref()
        .and_then(|context| context.loop_origins);
    let mut report = TxtpReport::default();
    let mut stems = StemSet::default();
    for trigger in triggers {
        let name = trigger.trigger().name();
        for variants in trigger
            .variants()
            .chunk_by(|a, b| std::ptr::eq(a.event(), b.event()))
        {
            let Some(first) = variants.first() else {
                continue;
            };
            let event = first.event();
            let breaks = trigger
                .breaks()
                .iter()
                .filter(|broken| std::ptr::eq(broken.event(), event))
                .count();
            let files = files_of(name, variants, &sounds, loop_origins);
            let written =
                event_text(name, event.name(), variants, &files, breaks).and_then(|rendered| {
                    let path = folder.join(format!(
                        "{}.{EXTENSION}",
                        stems.unique(&portable_stem(event.name(), FALLBACK_STEM))
                    ));
                    publish_event(
                        path,
                        name,
                        event.name(),
                        breaks,
                        rendered,
                        variants,
                        destination,
                    )
                });
            match written {
                Ok(file) => report.files.push(file),
                Err(reason) => report.skipped.push(TxtpSkip {
                    trigger: name.to_owned(),
                    event: comment_text(event.name()),
                    reason,
                }),
            }
        }
    }
    report
}

fn publish_event(
    path: PathBuf,
    trigger: &str,
    event: &str,
    breaks: usize,
    rendered: RenderedTxtp<'_>,
    variants: &[SoundVariant<'_>],
    destination: &Destination<'_>,
) -> Result<TxtpFile, String> {
    let publication = if let Some(context) = &destination.origins {
        let diagnostics = diagnostic::collect(&rendered, variants);
        output::write_with_origins(
            &path,
            &rendered.text,
            destination.policy,
            context,
            &OriginInputs {
                diagnostics: &diagnostics,
                loops: &rendered.loops,
            },
        )?
    } else {
        output::write(&path, &rendered.text, destination.policy)?
    };
    let command_loops = if publication.verified_content {
        rendered.command_loops()
    } else {
        None
    };
    Ok(TxtpFile {
        trigger: trigger.to_owned(),
        event: event.to_owned(),
        path,
        size: publication.size,
        state: publication.state,
        breaks,
        approximations: publication
            .verified_content
            .then_some(rendered.approximations),
        command_loops,
        origins: publication.origins,
        origin_receipt: publication.origin_receipt,
    })
}

/// Returns the TXTP text of one event, or why it cannot be written.
fn event_text<'a>(
    trigger: &str,
    event: &str,
    variants: &[SoundVariant<'_>],
    files: &HashMap<u32, SoundFile<'a>>,
    breaks: usize,
) -> Result<RenderedTxtp<'a>, String> {
    let tree = tree::build(variants).ok_or_else(|| "the event has no object path".to_owned())?;
    let mut header = vec![format!("P4KX: event {event} of trigger {trigger}")];
    if breaks > 0 {
        header.push(format!(
            "incomplete: {breaks} broken links; the objects behind them are not played here"
        ));
    }
    render(&tree, &header, files)
}

/// Returns the usable files of the WEMs the variants play: those written whole.
fn files_of<'a>(
    trigger: &str,
    variants: &[SoundVariant<'_>],
    sounds: &HashMap<(&str, u32), &'a ExportedSound>,
    loop_origins: Option<&'a ExportLoopOrigins>,
) -> HashMap<u32, SoundFile<'a>> {
    variants
        .iter()
        .filter_map(SoundVariant::wem_id)
        .filter_map(|wem_id| {
            let sound = sounds.get(&(trigger, wem_id))?;
            let name = sound.path.file_name()?.to_str()?;
            Some((
                wem_id,
                SoundFile {
                    name,
                    loop_points: sound.loop_points,
                    loop_origin: loop_origins.and_then(|origins| origins.for_sound(sound)),
                    media_origin: loop_origins.and_then(|origins| origins.media_for_sound(sound)),
                    prefetch_only: sound.prefetch_only,
                },
            ))
        })
        .collect()
}
