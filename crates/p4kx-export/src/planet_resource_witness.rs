// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Entry, SourceValueState};
use p4kx_testkit::real_archive_path;

use super::super::in_scope;
use super::TestResult;
use crate::catalog_read::entry_json_object::{self, EntryJsonObject};
use crate::catalog_read::loose_json::LooseJson;
use crate::catalog_read::original::{EntryCapture, with_entry_capture};
use crate::entry_source::EntrySource;
use crate::{CatalogError, ExportAccountingError};

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_planet_resources_have_the_four_observed_fields() -> TestResult {
    let archive = Archive::open(real_archive_path()?)?;
    let selectors_path =
        std::env::var_os("P4KX_PLANET_RESOURCE_LIST").ok_or("exact Entry selectors")?;
    let selectors = std::fs::read_to_string(selectors_path)?;
    let wanted: std::collections::HashSet<_> =
        selectors.lines().map(crate::starmap_index::key).collect();
    assert_eq!(wanted.len(), 24, "exact build-specific changed set");
    assert!(
        wanted.iter().all(|path| in_scope(path)),
        "all inputs are Resource definitions"
    );
    let build = archive.build().ok_or("installed Build identity")?;
    assert_eq!(build.change_number(), "13189690", "PTU change number");
    let mut count = 0;
    let mut empty_tint = 0;
    with_entry_capture(&archive, |capture| {
        for entry in archive
            .entries()
            .iter()
            .filter(|entry| wanted.contains(&crate::starmap_index::key(entry.path())))
        {
            empty_tint += usize::from(verified_empty_tint(&archive, entry, capture)?);
            count += 1;
        }
        Ok(())
    })?;
    assert_eq!(
        count,
        wanted.len(),
        "each exact original Entry was verified"
    );
    assert_eq!(empty_tint, 6, "explicit empty tint palette arguments");
    #[expect(
        clippy::print_stdout,
        reason = "ignored witness emits technical evidence counts only"
    )]
    {
        println!(
            "change={} verifiedResources={count} emptyTintPalettePath={empty_tint}",
            build.change_number()
        );
    }
    Ok(())
}

fn verified_empty_tint(
    archive: &Archive,
    entry: &Entry,
    capture: &EntryCapture<'_>,
) -> Result<bool, CatalogError> {
    let source = EntrySource::outer(capture, archive, entry)?;
    let Ok(read) = LooseJson::read(archive, entry, &source, None)? else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    let Ok(parsed) = read.parse(capture, None)? else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    let object = EntryJsonObject::new(&parsed, parsed.root, capture.source_uses())?
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    for state in [
        object.field("guid", entry_json_object::guid)?.state(),
        object.field("name", entry_json_object::text)?.state(),
        object
            .field("bMinable", entry_json_object::boolean)?
            .state(),
    ] {
        assert_eq!(
            state,
            SourceValueState::Present,
            "decoded sourced field state"
        );
    }
    let tint = object.field("tintPalettePath", entry_json_object::text)?;
    assert_eq!(
        tint.state(),
        SourceValueState::Present,
        "decoded tint path state"
    );
    Ok(tint.value().is_some_and(String::is_empty))
}
