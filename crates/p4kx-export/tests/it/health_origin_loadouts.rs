// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::health_origin_assertions as check;
use crate::health_origin_entry_source as entries;
use crate::health_origin_fixture::{Fixture, Input, TestResult};
use crate::health_origin_loadout_source as source;
use p4kx_assembly::{ArchiveFiles, LoadoutFileError, LoadoutFiles, MAX_XML_LOADOUT_LEN};
use p4kx_testkit::TestEntry;
use serde_json::{Value, json};

#[derive(Clone, Copy)]
enum Failure {
    Parse,
    Read,
    Size,
}
#[test]
fn loadout_failure_identities_keep_original_file_admission_across_guards_and_cached_callers()
-> TestResult {
    let bytes = source::database();
    let control = Fixture::open(&bytes, Input::Plain, Vec::new())?;
    source::cache_control(&control)?;
    for failure in [Failure::Parse, Failure::Read, Failure::Size] {
        let entry = match failure {
            Failure::Parse => TestEntry::stored(source::PATH, b"<broken>".as_slice()),
            Failure::Read => TestEntry::zstd(source::PATH, b"<Loadout/>".as_slice())
                .encrypted()
                .with_payload(vec![0, 1, 2]),
            Failure::Size => TestEntry::stored(source::PATH, b"<Loadout/>".as_slice())
                .encrypted()
                .with_uncompressed_size(MAX_XML_LOADOUT_LEN + 1),
        };
        let input = Fixture::open(&bytes, Input::Plain, vec![entry])?;
        let original = entries::original(&input.archive, source::PATH)?;
        let files = ArchiveFiles::new(&input.archive);
        let identity = files
            .identity("synthetic/shared.xml")
            .ok_or("actual original file admission")?;
        assert_eq!(
            identity.entry_index,
            Some(original.index()),
            "the same public resolution selects the actual original Entry"
        );
        assert_eq!(
            identity.path,
            original.path(),
            "the admitted path is copied from that original Entry"
        );
        original_outcome(&files, failure)?;
        let (_, catalog) = input.diagnostic()?;
        let operation = if matches!(failure, Failure::Parse) {
            "xmlLoadout"
        } else {
            "xmlLoadoutBytes"
        };
        let failed = entries::failure(&catalog, "loadouts", operation)?;
        assert_eq!(
            failed.len(),
            1,
            "physical file failures remain deduplicated independently of the two caller rows"
        );
        let row = failed.first().ok_or("actual failed XML operation")?;
        let path = check::present_text(&catalog, row, "path", &json!(source::PATH), Some(false))?;
        file_identity(&catalog, path)?;
        let field = check::present_text(&catalog, row, "field", &json!(source::PATH), Some(false))?;
        file_identity(&catalog, field)?;
        let source_type = check::present_text(
            &catalog,
            row,
            "sourceType",
            &json!("LoadoutFile"),
            Some(false),
        )?;
        check::independent(source_type)?;
        check::closure(&catalog)?;
    }
    Ok(())
}
fn original_outcome(files: &ArchiveFiles<'_>, failure: Failure) -> TestResult {
    match failure {
        Failure::Parse => assert_eq!(
            files.read("synthetic/shared.xml")?,
            b"<broken>",
            "the actual malformed parse input is readable"
        ),
        Failure::Read => assert!(
            matches!(
                files.read("synthetic/shared.xml"),
                Err(LoadoutFileError::Unreadable { .. })
            ),
            "the intended original byte read fails"
        ),
        Failure::Size => assert!(
            matches!(files.read("synthetic/shared.xml"), Err(LoadoutFileError::TooLarge { size }) if size == MAX_XML_LOADOUT_LEN + 1),
            "the original size guard fires before payload consumption"
        ),
    }
    Ok(())
}
fn file_identity(catalog: &Value, cell: &Value) -> TestResult {
    let uses = check::uses(catalog, cell)?;
    assert!(
        uses.iter().any(|used| {
            used.get("scope") == Some(&json!("archiveDirectoryMetadata"))
                && used.pointer("/location/field") == Some(&json!("path"))
                && check::source(catalog, used)
                    .is_ok_and(|source| source.pointer("/path/value") == Some(&json!(source::PATH)))
        }),
        "the original admitted Entry supplies the retained file identity"
    );
    for used in uses {
        if used.get("role") == Some(&json!("attempted"))
            || used.get("role") == Some(&json!("reference"))
        {
            continue;
        }
        if check::source(catalog, used)?.pointer("/path/value") == Some(&json!(source::PATH)) {
            assert_ne!(
                used.get("scope"),
                Some(&json!("payload")),
                "failed or guarded XML payload cannot taint copied file metadata"
            );
        }
    }
    Ok(())
}

#[path = "health_loadout_missing_body.rs"]
mod missing_body;

#[path = "health_loadout_mixed_vehicle.rs"]
mod mixed_vehicle;
