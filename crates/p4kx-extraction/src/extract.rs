// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_archive::{Archive, Entry};

use crate::buffers::Buffers;
use crate::dataforge_pick::{DataForgePick, Pick, located};
use crate::dependent::Collisions;
use crate::sound_job::mark_sounds;
use crate::sound_workers::{run_parallel, within};
use crate::source::{EntrySource, SourceEntry};
use crate::texture_job::{Item, group_textures};
use crate::tracker::Tracker;
use crate::write::{EntryTarget, EntryWriter};
use crate::{
    EntryFailure, ExtractOptions, ExtractionError, ExtractionReport, MAX_SOUND_LEN,
    MAX_SOUNDBANK_LEN,
};

pub(crate) use crate::extract_outcome::{Ended, EntryEnded};

/// Runs an Extraction of `entries` from `archive` into the existing folder `target`.
///
/// Files that already exist are handled by the [`ConflictPolicy`](crate::ConflictPolicy) of
/// `options`. Entries are written in parallel on the current rayon pool with the same files and
/// report as a sequential run in the order of `entries`. Entry paths that could leave `target` are
/// rejected, and existing links below `target` are never followed. A failed or cancelled Entry
/// leaves no partial file and keeps an existing one; once the token of `options` is cancelled,
/// the Entries not yet complete are reported as cancelled.
///
/// With Conversion on, each requested `.dds` header and its requested parts are converted once
/// as one texture as [`TextureConversion`](crate::TextureConversion) says, its parts read from
/// the Archive whether requested or not. A texture that cannot be converted is reported among
/// the [`unconverted`](ExtractionReport::unconverted) ones and its raw Entries are written. The
/// DataForge Entry, the one [`DataForgeEntry::locate`] finds, is written as one Record XML file
/// per Record below its folder after the other Entries, each file under the Conflict policy; its
/// Records are reported in [`records`](ExtractionReport::records).
///
/// With Conversion on, WEM sounds and the WEMs of soundbanks are written as
/// [`AudioConversion`](crate::AudioConversion) says, each soundbank also unchanged; a sound that
/// cannot be converted is reported among the [`unconverted`](ExtractionReport::unconverted) ones
/// and written unchanged. Sounds and soundbanks held in memory at once stay within 1 GiB; one
/// larger than 512 MiB is not read whole but written unchanged and reported as unconverted.
///
/// A change of the Archive seen by the watch of `options` stops the run like a cancel and is
/// reported. The watch is checked before the run, periodically during it, before each file is
/// put in place and after the run.
///
/// With [`ExtractOptions::threads`] set, the run, the sound workers and all work nested in them
/// use a rayon pool of at most that many threads, never more than the logical processors. If the
/// system refuses to start the pool, every Entry fails with [`ExtractionError::ThreadPool`] and
/// nothing is written.
#[must_use]
pub fn extract(
    archive: &Archive,
    entries: &[&Entry],
    target: &Path,
    options: &ExtractOptions<'_>,
) -> ExtractionReport {
    let files: Vec<SourceEntry<'_>> = entries
        .iter()
        .map(|entry| SourceEntry::of_archive(archive, entry))
        .collect();
    let rules = Rules {
        copy_other: true,
        entry_target: EntryTarget::Source,
        pick: &|entries, stop| located(archive, entries, stop),
    };
    extract_files(archive, &files, target, options, &rules)
}

/// What differs between an Extraction of an Archive and a Conversion of a folder.
pub(crate) struct Rules<'p> {
    /// Output naming independent of the actual source Entry identity.
    pub(crate) entry_target: EntryTarget<'p>,
    /// Whether a file that Conversion leaves unchanged is written too; without it such a file is
    /// not written but counted in [`ExtractionReport::not_converted`].
    pub(crate) copy_other: bool,
    /// How the DataForge to convert is chosen among the files.
    pub(crate) pick: Pick<'p>,
}

/// Runs an Extraction of `entries` of `source` like [`extract`], as `rules` say.
pub(crate) fn extract_files(
    source: &dyn EntrySource,
    entries: &[SourceEntry<'_>],
    target: &Path,
    options: &ExtractOptions<'_>,
    rules: &Rules<'_>,
) -> ExtractionReport {
    let Some(threads) = options.threads else {
        return run(source, entries, target, options, None, rules);
    };
    let limit = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads.get().min(limit))
        .thread_name(|number| format!("p4kx-extract-{number}"))
        .build();
    match pool {
        Ok(pool) => run(source, entries, target, options, Some(&pool), rules),
        Err(error) => ExtractionReport {
            failures: entries
                .iter()
                .map(|entry| EntryFailure {
                    path: entry.path().to_owned(),
                    error: ExtractionError::ThreadPool(error.to_string()),
                })
                .collect(),
            converted: options.convert.then_some(0),
            ..ExtractionReport::default()
        },
    }
}

fn run(
    source: &dyn EntrySource,
    entries: &[SourceEntry<'_>],
    target: &Path,
    options: &ExtractOptions<'_>,
    pool: Option<&rayon::ThreadPool>,
    rules: &Rules<'_>,
) -> ExtractionReport {
    if let Some(report) = not_started(entries, target, options) {
        return report;
    }
    let owned_protection = if options.protection.is_none() {
        source
            .archive_path()
            .map(|path| {
                p4kx_guard::ArchiveWriteProtection::new(
                    path,
                    crate::output_file::cancel_flag(options.cancel),
                )
            })
            .transpose()
    } else {
        Ok(None)
    };
    let owned_protection = match owned_protection {
        Ok(protection) => protection,
        Err(error) => return refused_report(entries, options, &error),
    };
    let protection = options.protection.or(owned_protection.as_ref());
    if let Err(error) = crate::check_protected_output(target, protection, options.cancel) {
        return refused_report(entries, options, &error);
    }
    run_guarded(source, entries, target, options, pool, rules, protection)
}

fn run_guarded(
    source: &dyn EntrySource,
    entries: &[SourceEntry<'_>],
    target: &Path,
    options: &ExtractOptions<'_>,
    pool: Option<&rayon::ThreadPool>,
    rules: &Rules<'_>,
    protection: Option<&p4kx_guard::ArchiveWriteProtection>,
) -> ExtractionReport {
    let tracker = Tracker::new(entries, options.cancel, options.watch, options.progress);
    let absolute = std::path::absolute(target).unwrap_or_else(|_| target.to_path_buf());
    let writer = EntryWriter {
        entry_target: rules.entry_target,
        protection,
        cancel: options.cancel,
        source,
        target: &absolute,
        policy: options.policy,
        convert: options.convert,
        copy_other: rules.copy_other,
        textures: options.textures,
        audio: options.audio,
        verify: options.verify,
        tracker: &tracker,
    };
    let found = if options.convert {
        within(pool, || (rules.pick)(entries, &|| tracker.should_stop()))
    } else {
        DataForgePick::default()
    };
    let dataforge = found.position;
    let (independent, dependent) =
        split_dependent(within(pool, || items(source, entries, options, dataforge)));
    let run = |buffers: &mut Buffers, item: Item<'_>| {
        let ended = write_item(&writer, item, buffers);
        writer.record_written(entries, &ended);
        ended
    };
    let mut ended = run_parallel(independent, &run, pool, &|| tracker.should_stop());
    let mut buffers = Buffers::new();
    let later = within(pool, || {
        dependent
            .into_iter()
            .map(|item| run(&mut buffers, item))
            .collect::<Vec<_>>()
    });
    ended.extend(later);
    let records = dataforge
        .and_then(|index| entries.get(index).map(|entry| (index, entry)))
        .and_then(|(index, entry)| {
            let (outcome, records) = within(pool, || writer.write_dataforge(entry, &mut buffers));
            ended.push(Ended::Entry((index, entry.path().to_owned(), outcome)));
            records
        });
    let mut report = crate::extract_outcome::summarise(ended, options.convert);
    report.records = records;
    report.records_entry = dataforge
        .and_then(|index| entries.get(index))
        .map(|entry| entry.path().to_owned());
    report.dataforge_left_out = found
        .left_out
        .iter()
        .filter_map(|&position| entries.get(position))
        .map(|entry| entry.path().to_owned())
        .collect();
    report.archive_changed = options.watch.is_some_and(|watch| watch.check().is_err());
    report
}

fn refused_report(
    entries: &[SourceEntry<'_>],
    options: &ExtractOptions<'_>,
    error: &p4kx_guard::WriteProtectionError,
) -> ExtractionReport {
    if error.is_cancelled() {
        return ExtractionReport {
            cancelled: entries
                .iter()
                .map(|entry| entry.path().to_owned())
                .collect(),
            converted: options.convert.then_some(0),
            ..ExtractionReport::default()
        };
    }
    ExtractionReport {
        failures: entries
            .iter()
            .map(|entry| EntryFailure {
                path: entry.path().to_owned(),
                error: ExtractionError::Installation(error.clone()),
            })
            .collect(),
        converted: options.convert.then_some(0),
        ..ExtractionReport::default()
    }
}

/// Writes one item of the run and returns how it ended.
fn write_item(writer: &EntryWriter<'_>, item: Item<'_>, buffers: &mut Buffers) -> Ended {
    match item {
        Item::Entry((index, entry)) => {
            Ended::Entry((index, entry.path().to_owned(), writer.write(entry, buffers)))
        }
        Item::Texture(job) => writer.write_texture(&job, buffers),
        Item::Sound(indexed) => writer.convert_sound_entry(
            indexed,
            buffers,
            MAX_SOUND_LEN,
            EntryWriter::try_write_sound,
        ),
        Item::Bank(indexed) => writer.convert_sound_entry(
            indexed,
            buffers,
            MAX_SOUNDBANK_LEN,
            EntryWriter::try_write_bank,
        ),
    }
}

/// Returns the report of a run that must not start: every Entry failed because `target` lies
/// inside a Star Citizen installation, or was cancelled because the Archive already changed.
fn not_started(
    entries: &[SourceEntry<'_>],
    target: &Path,
    options: &ExtractOptions<'_>,
) -> Option<ExtractionReport> {
    let converted = options.convert.then_some(0);
    if options.cancel.is_some_and(crate::CancelToken::is_cancelled) {
        return Some(ExtractionReport {
            cancelled: entries
                .iter()
                .map(|entry| entry.path().to_owned())
                .collect(),
            converted,
            ..ExtractionReport::default()
        });
    }
    if let Err(refused) = p4kx_guard::ensure_writable(target) {
        let failures = entries.iter().map(|entry| EntryFailure {
            path: entry.path().to_owned(),
            error: ExtractionError::Installation(refused.clone()),
        });
        return Some(ExtractionReport {
            failures: failures.collect(),
            converted,
            ..ExtractionReport::default()
        });
    }
    options
        .watch
        .is_some_and(|watch| watch.check().is_err())
        .then(|| ExtractionReport {
            cancelled: entries
                .iter()
                .map(|entry| entry.path().to_owned())
                .collect(),
            converted,
            archive_changed: true,
            ..ExtractionReport::default()
        })
}

/// Splits `entries` into the items of the run: with Conversion on, textures, sounds and
/// soundbanks; the DataForge Entry at `dataforge` is left out.
fn items<'a>(
    source: &'a dyn EntrySource,
    entries: &'a [SourceEntry<'a>],
    options: &ExtractOptions<'_>,
    dataforge: Option<usize>,
) -> Vec<Item<'a>> {
    let mut items = if options.convert && options.textures.converts() {
        group_textures(source, entries)
    } else {
        entries.iter().enumerate().map(Item::Entry).collect()
    };
    items.retain(|item| !matches!(item, Item::Entry((index, _)) if Some(*index) == dataforge));
    if options.convert {
        mark_sounds(&mut items, options.audio);
    }
    items
}

/// Splits `items` into those that may run in parallel and, in order, those that write a path
/// colliding with an earlier one by name or as a file where a folder is needed.
fn split_dependent(items: Vec<Item<'_>>) -> (Vec<Item<'_>>, Vec<Item<'_>>) {
    let mut collisions = Collisions::default();
    items.into_iter().partition(|item| {
        let mut free = true;
        item.output_paths(|path| free &= collisions.claim(path));
        free
    })
}
