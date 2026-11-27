// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::TestEntry;

use super::{Limits, child, external_at, read, root, system};
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::SourceDisposition;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn unbudgeted_reused_containers_keep_every_authored_placement() -> TestResult {
    let dir = tempfile::tempdir()?;
    let placements = read(dir.path(), reused(), Limits::default());
    assert_eq!(placements.counts.malformed, 0, "no route quota");
    assert_eq!(
        placements.counts.ambiguous, 1,
        "both repeated Record placements survive"
    );
    assert!(
        placements.ledger.incomplete_inventories.is_empty(),
        "unlimited traversal keeps complete inventory"
    );
    assert_eq!(
        Limits::default().source_children,
        None,
        "no default physical source quota"
    );
    assert_eq!(
        Limits::default().placed_children_per_system,
        None,
        "no default placement visit quota"
    );
    Ok(())
}

#[test]
fn physical_and_instantiated_child_allowances_are_independent() -> TestResult {
    for (source, routes, truncated) in [
        (Some(3), Some(4), false),
        (Some(3), Some(3), true),
        (Some(2), None, true),
    ] {
        let dir = tempfile::tempdir()?;
        let limits = Limits {
            source_children: source,
            placed_children_per_system: routes,
        };
        let placements = read(dir.path(), reused(), limits);
        assert_eq!(placements.counts.malformed > 0, truncated);
        assert_eq!(
            placements.ledger.incomplete_inventories.is_empty(),
            !truncated
        );
        let failed_source = placements.ledger.entries.iter().any(|entry| {
            entry.operation == EntryOperation::ContainerXml
                && entry.disposition == SourceDisposition::ProjectionFailed
        });
        assert_eq!(
            failed_source,
            source == Some(2),
            "physical reads stay distinct from placement visits"
        );
    }
    Ok(())
}

fn reused() -> Vec<TestEntry> {
    system(
        &root(vec![
            external_at("shared", "10,0,0"),
            external_at("shared", "20,0,0"),
        ]),
        &[("shared", root(vec![child("1,0,0", 1)]))],
    )
}
