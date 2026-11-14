// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_search::{Needle, SearchOptions};

use crate::complete_lines_source::{self as source, Checked};

#[test]
fn complete_lines_keep_a_single_cancellation_request_authoritative() -> Checked {
    let mut bytes = b"not a match\n".repeat(1024);
    bytes.extend_from_slice(b"needle");
    assert_eq!(
        std::str::from_utf8(&bytes)?.lines().count(),
        1025,
        "the independent source has enough original lines"
    );
    let fixture = source::fixture(&bytes)?;
    let polls = AtomicUsize::new(0);
    let found = p4kx_search::search_entries(
        &fixture.archive,
        &Needle::new("needle")?,
        &SearchOptions::default(),
        &|| polls.fetch_add(1, Ordering::Relaxed) == 2,
    );
    assert!(
        polls.load(Ordering::Relaxed) >= 3,
        "the actual caller did receive its one true cancellation response"
    );
    assert!(
        found.is_none(),
        "a later false callback cannot restore an apparently complete result after cancellation"
    );
    Ok(())
}
