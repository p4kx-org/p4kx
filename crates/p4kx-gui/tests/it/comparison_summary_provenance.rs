// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;

use p4kx_comparison::{CancelToken, EntrySummaryProvenance};
use p4kx_extraction::{FieldPath, PhysicalSourceKind};
use p4kx_gui::Language;
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};

use crate::comparing::{self, Pair, TestResult};
use crate::comparison_model::{compared, filtered, picked};

#[test]
fn entry_summary_provenance_follows_filtered_members_and_spelling() -> TestResult {
    let pair = entries_with_mixed_checksums()?;
    let mut model = picked(&pair, Language::BritishEnglish);
    let cancel = CancelToken::new();
    let view = compared(&mut model)?;
    assert_eq!(view.summary().incomparables, 2);
    assert_eq!(view.summary().directories[0].directory, "Data");
    let initial = view
        .entry_summary_provenance(&cancel)?
        .ok_or("initial summary origins")?;
    assert_eq!(
        entry_paths(&initial, "/entries/directories/0/directory")?,
        ["Data/a.txt"],
        "unfiltered directory spelling comes from the first member"
    );

    view.set_filter("keep", || {});
    filtered(&mut model)?;
    let view = model.comparison().view().ok_or("no filtered view")?;
    assert_eq!(view.summary().counts.changed, 1);
    assert_eq!(view.summary().incomparables, 1);
    assert_eq!(view.summary().directories[0].directory, "data");
    let filtered_origins = view
        .entry_summary_provenance(&cancel)?
        .ok_or("filtered summary origins")?;
    assert_eq!(
        entry_paths(&filtered_origins, "/entries/directories/0/directory")?,
        ["data/b-keep.txt"],
        "filtered spelling uses the first retained member"
    );
    assert_eq!(
        entry_paths(&filtered_origins, "/entries/counts/changed")?,
        ["DATA/b-keep.txt", "data/b-keep.txt"],
        "the selected changed row retains both original inputs"
    );
    assert_eq!(
        entry_paths(&filtered_origins, "/entries/incomparableCount")?,
        ["Data/incomparable-keep.txt"; 2],
        "the incomparable counter follows the same filter"
    );
    cancel.cancel();
    assert!(
        view.entry_summary_provenance(&cancel)?.is_none(),
        "cancelled requests return no evidence"
    );
    Ok(())
}

fn entries_with_mixed_checksums() -> Result<Pair, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let old = dir.path().join("old.zip");
    let checksum_pair = || TestEntry::stored("Data/incomparable-keep.txt", b"same");
    let other_checksum_pair = || TestEntry::stored("Data/incomparable.txt", b"same");
    let zip = TestContainer::new()
        .entry(TestEntry::stored("Data/a.txt", b"old"))
        .entry(TestEntry::stored("DATA/b-keep.txt", b"old"))
        .entry(checksum_pair())
        .entry(other_checksum_pair())
        .build();
    fs::write(&old, zip)?;
    let p4k = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/a.txt", b"new size"))
        .entry(TestEntry::stored("data/b-keep.txt", b"new size"))
        .entry(checksum_pair())
        .entry(other_checksum_pair());
    let new = comparing::write(dir.path(), "PTU", "4.3.2.10400", &p4k)?;
    Ok(Pair { dir, old, new })
}

fn entry_paths<'evidence>(
    evidence: &'evidence EntrySummaryProvenance,
    path: &str,
) -> Result<Vec<&'evidence str>, Box<dyn Error>> {
    let origin = evidence
        .field_origins()
        .get(&FieldPath::parse(path)?)
        .ok_or("field origin")?;
    let selected = evidence
        .source_inventory()
        .select_uses(origin.source_uses_refs())?;
    let mut paths = Vec::new();
    for source in evidence.source_inventory().iter_sources() {
        if selected.iter_sources().any(|used| used.id() == source.id())
            && let PhysicalSourceKind::ArchiveEntry(entry) = source.kind()
            && let Some(path) = entry.path().value()
        {
            paths.push(path.as_str());
        }
    }
    paths.sort_unstable();
    Ok(paths)
}
