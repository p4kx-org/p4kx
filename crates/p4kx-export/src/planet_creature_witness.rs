// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, SourceValueState};
use p4kx_testkit::real_archive_path;

use super::super::FAMILY;
use super::TestResult;
use crate::ExportAccountingError;
use crate::catalog_read::entry_json_object::{self, EntryJsonObject};
use crate::catalog_read::loose_json::LooseJson;
use crate::catalog_read::original::with_entry_capture;
use crate::entry_source::EntrySource;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_fauna_definitions_have_the_exact_observed_field_shapes() -> TestResult {
    let archive = Archive::open(real_archive_path()?)?;
    let manifest = archive.build().ok_or("installed Build receipt")?;
    assert_eq!(manifest.change_number(), "13189690", "exact PTU changelist");
    let selector_file =
        std::env::var_os("P4KX_CREATURE_WITNESS_PATHS").ok_or("private Entry selectors")?;
    let selectors: std::collections::HashSet<_> = std::fs::read_to_string(selector_file)?
        .lines()
        .map(crate::starmap_index::key)
        .collect();
    assert_eq!(selectors.len(), 6, "PTU creature additions");
    assert!(
        selectors.iter().all(|path| FAMILY.includes(path)),
        "exact fauna Entry scope"
    );
    let mut total = 0;
    with_entry_capture(&archive, |capture| {
        for entry in archive
            .entries()
            .iter()
            .filter(|entry| selectors.contains(&crate::starmap_index::key(entry.path())))
        {
            let source = EntrySource::outer(capture, &archive, entry)?;
            let Ok(read) = LooseJson::read(&archive, entry, &source, None)? else {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            };
            let Ok(parsed) = read.parse(capture, None)? else {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            };
            let object = EntryJsonObject::new(&parsed, parsed.root, capture.source_uses())?
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            for state in [
                object.field("guid", entry_json_object::guid)?.state(),
                object
                    .field("entityClass", entry_json_object::guid)?
                    .state(),
                object
                    .field("needVerticalOffset", entry_json_object::boolean)?
                    .state(),
                object
                    .field("slopeTolerance", entry_json_object::number)?
                    .state(),
            ] {
                assert_eq!(
                    state,
                    SourceValueState::Present,
                    "exact authored field type"
                );
            }
            total += 1;
        }
        Ok(())
    })?;
    assert_eq!(
        total,
        selectors.len(),
        "all expected Entries are CRC verified and projected"
    );
    Ok(())
}
