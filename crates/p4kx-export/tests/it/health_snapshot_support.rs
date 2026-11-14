// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::{
    InputScope, PhysicalSource, SourceInput, SourceInventory, SourceLocation, SourceScope,
    SourceUses,
};

use crate::health_origin_fixture::{self as fixture, Fixture, TestResult};

pub(crate) fn closed(inventory: &SourceInventory) {
    let sources: BTreeSet<_> = inventory.sources().iter().map(PhysicalSource::id).collect();
    assert!(
        !sources.is_empty(),
        "the actual observed DataForge input is retained in its result"
    );
    assert_eq!(
        sources.len(),
        inventory.sources().len(),
        "original source IDs are unique within this snapshot"
    );
    let mut uses = BTreeSet::new();
    for observed in inventory.uses() {
        assert!(
            sources.contains(&observed.source()),
            "every actual use resolves in this result's own snapshot"
        );
        assert!(
            uses.insert(observed.id()),
            "original use IDs are unique within this snapshot"
        );
    }
}

pub(crate) fn registry(input: &Fixture) -> TestResult<(SourceUses, SourceScope, String)> {
    let forge = input.entry.open()?;
    let record = forge.record(0).ok_or("actual original header owner")?;
    assert_eq!(
        record.index(),
        0,
        "the source coordinate uses its actual physical position"
    );
    assert_eq!(
        record.guid().to_string(),
        fixture::guid(fixture::LIST),
        "the actual original GUID is retained before binding"
    );
    let entry = input
        .archive
        .entries()
        .first()
        .ok_or("actual original DCB Entry")?;
    let observed = input
        .archive
        .observed_entry(entry)
        .ok_or("actual original directory admission")?;
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::ArchiveView(observed), InputScope::Payload)?;
    Ok((sources, scope, record.guid().to_string()))
}

pub(crate) fn guid_location() -> SourceLocation {
    SourceLocation::Subresource {
        namespace: "dataforgeRecordHeader".to_owned(),
        indices: vec![0],
        field: "guid".to_owned(),
    }
}

pub(crate) fn broken_subject(input: &Fixture) -> TestResult {
    let forge = input.entry.open()?;
    let record = forge.record(0).ok_or("actual original failed subject")?;
    assert!(
        record.tree().is_err(),
        "the fixture has its actual original source failure"
    );
    Ok(())
}
