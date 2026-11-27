// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::health_origin_assertions as check;
use crate::health_origin_entry_source as entries;
use crate::health_origin_fixture::{self as fixture, Fixture, Input, TestResult};
use p4kx_testkit::TestEntry;
use serde_json::{Value, json};

#[test]
fn unreadable_encrypted_entry_payload_does_not_taint_its_plain_directory_identity() -> TestResult {
    let broken = TestEntry::zstd(fixture::REGISTRY, b"<LayerBackups/>".as_slice())
        .encrypted()
        .with_payload(vec![0, 1, 2]);
    let source = Fixture::open(&fixture::empty_database(), Input::Plain, vec![broken])?;
    let entry = entries::original(&source.archive, fixture::REGISTRY)?;
    assert_eq!(
        entry.path(),
        fixture::REGISTRY,
        "the original stored directory path remains readable"
    );
    assert!(
        entry.is_encrypted(),
        "the actual selected leaf is marked encrypted"
    );
    assert!(
        entries::read(&source.archive, entry).is_err(),
        "the intended original byte read really fails"
    );
    let (_, catalog) = source.diagnostic()?;
    let failed = entries::failure(&catalog, "shops", "shopRegistry")?;
    assert_eq!(failed.len(), 1, "one original registry read failed");
    let failed = failed.first().ok_or("failed registry")?;
    let path = check::present_text(
        &catalog,
        failed,
        "path",
        &json!(fixture::REGISTRY),
        Some(false),
    )?;
    assert_eq!(
        entries::metadata_paths(&catalog, path)?,
        [fixture::REGISTRY],
        "only that original directory path contributes"
    );
    let index = check::present(failed, "entryIndex", &json!(entry.index()), Some(false))?;
    original_index(&catalog, index, entry.index())?;
    let target = check::cell(failed, "targetGuid", "absent")?;
    check::independent(target)?;
    let inventory = check::rows(&catalog, "/provenance/uses")?;
    assert!(
        inventory
            .iter()
            .any(|used| used.get("scope") == Some(&json!("payload"))
                && used.get("role") == Some(&json!("attempted"))
                && check::source(&catalog, used).is_ok_and(
                    |source| source.pointer("/path/value") == Some(&json!(fixture::REGISTRY))
                )),
        "the failed payload remains an attempt in the actual inventory"
    );
    check::closure(&catalog)
}

fn original_index(catalog: &Value, cell: &Value, index: usize) -> TestResult {
    let uses = check::uses(catalog, cell)?;
    assert_eq!(
        uses.len(),
        1,
        "the retained Entry position has one actual directory selector"
    );
    let contribution = uses.first().ok_or("actual Entry-index selector")?;
    assert_eq!(
        contribution.get("role"),
        Some(&json!("selector")),
        "the original position is a selection fact"
    );
    assert_eq!(
        contribution.get("scope"),
        Some(&json!("archiveDirectoryMetadata")),
        "the position does not consume failed encrypted payload"
    );
    assert_eq!(
        contribution.pointer("/location/type"),
        Some(&json!("metadata")),
        "the physical position uses a directory coordinate"
    );
    assert_eq!(
        contribution.pointer("/location/field"),
        Some(&json!("entryIndex")),
        "the actual original index is selected"
    );
    let source = check::source(catalog, contribution)?;
    assert_eq!(
        source.get("kind"),
        Some(&json!("archiveEntry")),
        "the selector belongs to the original admitted Entry"
    );
    assert_eq!(
        source.get("entryIndex"),
        Some(&json!(index)),
        "the original physical Entry slot is unchanged"
    );
    assert_eq!(
        source.pointer("/path/value"),
        Some(&json!(fixture::REGISTRY)),
        "an unrelated directory Entry cannot certify this retained position"
    );
    Ok(())
}
