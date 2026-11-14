// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Ordering;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

use p4kx_extraction::{Archive, Entry, Language};
use rayon::prelude::*;

use crate::entry_scan::{Context, Outcome, scan};
use crate::gate::Gate;
use crate::waves::{Chunk, collect};
use crate::{EntryHit, Found, Needle, SearchOptions, Source};

/// The extensions of the Entries searched when no Filter names others: text and `CryXml` formats.
pub const TEXT_EXTENSIONS: [&str; 15] = [
    "xml",
    "mtl",
    "cdf",
    "adb",
    "entxml",
    "chrparams",
    "animevents",
    "ini",
    "cfg",
    "json",
    "txt",
    "lua",
    "csv",
    "eco",
    "tbl",
];

const ENTRIES_PER_TASK: usize = 16;
const MEMORY_BUDGET: u64 = 256 << 20;

/// Searches the text of the Entries of `archive`, `CryXml` converted to XML first.
///
/// Without a [`SearchOptions::filter`] the Entries with one of the [`TEXT_EXTENSIONS`] are
/// searched, and with one every Entry whose path matches; the `global.ini` files are left to
/// [`search_localization`](crate::search_localization) when [`SearchOptions::sources`] holds
/// [`Source::Localization`]. Entries are searched in path order in parallel waves: plain text
/// is matched incrementally without retaining a whole line. `CryXml` and UTF-16 are read whole
/// under a 256 MiB admission budget counting literal state, excerpts, parsed `CryXml` tables
/// and the buffer that decodes UTF-16.
/// A hit names the line, counted in the XML form for `CryXml`; an Entry reports at most
/// [`SearchOptions::per_item`] hits. An Entry above [`SearchOptions::max_entry_len`], one that
/// holds binary data, an encrypted one without the Archive key and one that cannot be read are
/// skipped and counted. Returns `None` once `stop` has returned `true`.
#[must_use]
pub fn search_entries(
    archive: &Archive,
    needle: &Needle,
    options: &SearchOptions,
    stop: &(dyn Fn() -> bool + Sync),
) -> Option<Found<EntryHit>> {
    let cancelled = AtomicBool::new(false);
    let stopped = || {
        if cancelled.load(AtomicOrdering::Relaxed) {
            return true;
        }
        if stop() {
            cancelled.store(true, AtomicOrdering::Relaxed);
            return true;
        }
        false
    };
    let stop: &(dyn Fn() -> bool + Sync) = &stopped;
    let candidates = candidates(archive, options);
    let gate = Gate::new(MEMORY_BUDGET);
    let context = Context {
        needle,
        per_item: options.item_limit(),
        max_len: options.max_entry_len,
        gate: &gate,
        stop,
    };
    let limit = options.hit_limit();
    let tasks = candidates.len().div_ceil(ENTRIES_PER_TASK);
    collect(tasks, limit, stop, |task| {
        let start = task * ENTRIES_PER_TASK;
        let slice = candidates
            .get(start..candidates.len().min(start + ENTRIES_PER_TASK))
            .unwrap_or_default();
        let mut chunk = Chunk::default();
        let mut buffer = Vec::new();
        for entry in slice {
            if stop() || chunk.hits.len() > limit {
                break;
            }
            match scan(archive, entry, &context, &mut buffer) {
                Outcome::Scanned(hits) => {
                    chunk.stats.entries_scanned += 1;
                    chunk.hits.extend(hits);
                }
                Outcome::Binary => chunk.stats.entries_binary += 1,
                Outcome::TooLarge => chunk.stats.entries_too_large += 1,
                Outcome::KeyRequired => chunk.stats.entries_key_required += 1,
                Outcome::Unreadable => chunk.stats.entries_unreadable += 1,
                Outcome::Cancelled => break,
            }
        }
        chunk
    })
}

fn candidates<'a>(archive: &'a Archive, options: &SearchOptions) -> Vec<&'a Entry> {
    let leave_ini = options.sources.contains(Source::Localization);
    let mut found: Vec<&Entry> = archive
        .entries()
        .par_iter()
        .filter(|entry| {
            let path = entry.path();
            let selected = options
                .filter
                .as_ref()
                .map_or_else(|| has_text_extension(path), |filter| filter.matches(path));
            selected
                && !options.exclusion.matches(path)
                && !(leave_ini && Language::from_global_ini_path(path).is_some())
        })
        .collect();
    found.par_sort_unstable_by(|left, right| path_order(left.path(), right.path()));
    found
}

fn has_text_extension(path: &str) -> bool {
    let name = path.rsplit(['\\', '/']).next().unwrap_or(path);
    name.rsplit_once('.').is_some_and(|(_, extension)| {
        TEXT_EXTENSIONS
            .iter()
            .any(|known| extension.eq_ignore_ascii_case(known))
    })
}

fn path_order(left: &str, right: &str) -> Ordering {
    let key = |byte: u8| {
        if byte == b'/' {
            b'\\'
        } else {
            byte.to_ascii_lowercase()
        }
    };
    left.bytes()
        .map(key)
        .cmp(right.bytes().map(key))
        .then_with(|| left.cmp(right))
}
