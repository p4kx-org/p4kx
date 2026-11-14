// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Content search service of P4KX: text inside Records, localisation and Entries
//! (ADR-0044).
//!
//! A search is a parallel scan on demand, with no index kept anywhere. [`search_records`] reads
//! the canonical tree of every Record and matches its string, localisation key and enum values;
//! [`search_localization`] matches the texts of each language's `global.ini`; [`search_entries`]
//! matches the lines of text and `CryXml` Entries. [`search`] runs the sources a
//! [`SearchOptions`] names over an Archive. Matching is a case-insensitive substring match of a
//! [`Needle`].
//!
//! Every source visits its items in a fixed order and appends the results of parallel tasks in
//! that order, so the hits do not depend on the number of threads. A source reports at most
//! [`SearchOptions::limit`] hits and says whether more exist, polls a stop function and holds a
//! bounded amount of memory: plain text is streamed, `CryXml` and UTF-16 Entries are read whole
//! under a memory budget, and one language's `global.ini` is held at a time. Text in a hit is an
//! [`Excerpt`] that is safe to print.

mod entries;
mod entry_scan;
mod excerpt;
mod gate;
mod line_excerpt;
mod line_match;
mod line_sink;
#[cfg(test)]
mod line_stream_tests;
mod localization;
mod needle;
mod needle_scan;
mod options;
mod records;
mod report;
mod search;
mod waves;

pub use entries::{TEXT_EXTENSIONS, search_entries};
pub use excerpt::{Excerpt, display_text};
pub use localization::search_localization;
pub use needle::{EmptyNeedle, Needle};
pub use options::{
    DEFAULT_LIMIT, DEFAULT_PER_ITEM, MAX_ENTRY_LEN, MAX_LIMIT, SearchOptions, Source, Sources,
};
pub use records::search_records;
pub use report::{
    EntryHit, Found, LocalizationHit, RecordHit, SearchReport, SearchStats, Unavailable,
};
pub use search::{SearchError, search, search_in};
