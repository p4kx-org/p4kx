// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_search::{self as search, Needle, SearchOptions};

use crate::complete_lines_source::{self as source, CUT, Checked};

#[test]
fn complete_lines_preserve_explicit_size_hit_limits_and_cancellation() -> Checked {
    let fixture = source::fixture(b"needle first\nneedle second\n")?;
    let per_item = SearchOptions {
        per_item: 1,
        ..SearchOptions::default()
    };
    let first = source::run(&fixture, "needle", &per_item)?;
    assert_eq!(
        first.hits.len(),
        1,
        "the explicit perEntry hit limit remains binding"
    );
    assert_eq!(
        source::hit(&first, 0)?.line,
        1,
        "the first original matching line wins"
    );
    let global = SearchOptions {
        limit: 1,
        ..SearchOptions::default()
    };
    let limited = source::run(&fixture, "needle", &global)?;
    assert_eq!(
        limited.hits.len(),
        1,
        "the explicit global result limit remains binding"
    );
    assert!(
        limited.has_more,
        "the actual second matching line proves more results"
    );
    let refused = SearchOptions {
        max_entry_len: 1,
        ..SearchOptions::default()
    };
    let too_large = source::run(&fixture, "needle", &refused)?;
    assert!(
        too_large.hits.is_empty(),
        "explicit Entry-size refusal returns no made-up matches"
    );
    assert_eq!(
        too_large.stats.entries_too_large, 1,
        "size refusal stays visible"
    );
    let long = source::fixture("x".repeat(CUT + 37).as_bytes())?;
    let cancelled = search::search_entries(
        &long.archive,
        &Needle::new("needle")?,
        &SearchOptions::default(),
        &|| true,
    );
    assert!(
        cancelled.is_none(),
        "actual cancellation never returns partial complete results"
    );
    Ok(())
}
