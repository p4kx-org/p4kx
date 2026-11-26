// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;

use p4kx_extraction::{
    Archive, InputScope, ProvenanceMode, SourceInput, SourceLocation, SourceUse, SourceUseRole,
    SourceUses,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::{Contributions, MarkerPortPrefix};
use crate::build::original_input::test_support::TestResult;
use crate::{CatalogError, ExportAccountingError};

#[test]
fn port_marker_prefix_reads_only_new_sites_and_keeps_earlier_states() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Plain.xml", "plain"))
        .entry(TestEntry::stored("Data/Protected.xml", "protected").encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;

    let sources = SourceUses::with_provenance_mode(ProvenanceMode::MarkersOnly);
    let uses = document_uses(&sources, &archive)?;
    assert_prefix_states(&sources, &uses)
}

fn document_uses(sources: &SourceUses, archive: &Archive) -> TestResult<[SourceUse; 3]> {
    let [plain, protected] = archive.entries() else {
        return Err("two physical vehicle documents".into());
    };
    let plain = archive.entry_origin(plain).ok_or("plain document origin")?;
    let protected = archive
        .entry_origin(protected)
        .ok_or("protected document origin")?;
    let plain = sources.observe(SourceInput::Archive(&plain), InputScope::Payload)?;
    let protected = sources.observe(SourceInput::Archive(&protected), InputScope::Payload)?;

    let selection = || SourceLocation::Subresource {
        namespace: "vehicleXml".to_owned(),
        indices: vec![0],
        field: "name".to_owned(),
    };
    Ok([
        plain.at(selection(), SourceUseRole::Selector)?,
        protected.at(selection(), SourceUseRole::Attempted)?,
        protected.at(selection(), SourceUseRole::Selector)?,
    ])
}

fn assert_prefix_states(sources: &SourceUses, uses: &[SourceUse]) -> TestResult {
    let mut prefix = MarkerPortPrefix::new();
    let choices = Cell::new(0);
    let consulted = Cell::new(0);
    for (count, expected_marker, complete) in [
        (1, Some(false), true),
        (3, Some(true), false),
        (2, None, false),
        (0, Some(false), true),
        (3, Some(true), false),
    ] {
        let flat = uses.get(..count).ok_or("flat reference prefix")?;
        let flat = sources.origin(flat, complete);
        let actual = prefix.through(
            count,
            sources,
            || {
                choices.set(choices.get() + 1);
                Ok(Contributions::default())
            },
            |index| bind_site(index, uses, &consulted),
        )?;
        assert_eq!(
            actual.contains_encrypted_source(),
            expected_marker,
            "prefix {count} keeps the exact marker"
        );
        assert_eq!(
            serde_json::to_value(actual)?,
            serde_json::to_value(flat)?,
            "prefix {count} matches the ordered flat proof"
        );
        assert_eq!(
            actual.source_set_complete(),
            complete,
            "prefix {count} preserves internal source-set completeness"
        );
    }
    assert_eq!(choices.get(), 1, "file choices are observed once per node");
    assert_eq!(
        consulted.get(),
        3,
        "each consulted original site is bound once"
    );
    let result = prefix.through(
        4,
        sources,
        || Err(ExportAccountingError::ConflictingSourceDatum.into()),
        |index| bind_site(index, uses, &consulted),
    );
    assert!(
        matches!(
            result,
            Err(CatalogError::Accounting(
                ExportAccountingError::UnobservedFailure
            ))
        ),
        "an unobserved suffix errors only when that suffix is requested"
    );
    Ok(())
}

fn bind_site(
    index: usize,
    uses: &[SourceUse],
    consulted: &Cell<usize>,
) -> Result<Contributions, CatalogError> {
    assert_eq!(index, consulted.get(), "original selector traversal order");
    let contribution = uses
        .get(index)
        .ok_or(ExportAccountingError::UnobservedFailure)?;
    consulted.set(consulted.get() + 1);
    let complete = contribution.role() != SourceUseRole::Attempted;
    Ok(Contributions::new(vec![contribution.clone()], complete))
}
