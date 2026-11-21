// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::health_origin_assertions as check;
use crate::health_origin_fixture::{self as fixture, Fixture, Input, TestResult};
use p4kx_extraction::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

#[test]
fn health_metadata_preserves_getter_errors_empty_values_and_all_physical_guid_aliases() -> TestResult
{
    aliases_and_bounded_examples()?;
    for state in [
        Metadata::EmptyFile,
        Metadata::UnreadableFile,
        Metadata::UnreadableType,
    ] {
        let bytes = metadata(state)?;
        original(&bytes, state)?;
        let source = Fixture::open(&bytes, Input::Plain, Vec::new())?;
        let (_, catalog) = source.diagnostic()?;
        let failed = check::rows(&catalog, "/exportHealth/failedRecords")?
            .first()
            .ok_or("actual metadata failure")?;
        check::present(
            failed,
            "guid",
            &json!(fixture::guid(fixture::LIST)),
            Some(false),
        )?;
        match state {
            Metadata::EmptyFile => {
                check::present_text(&catalog, failed, "recordPath", &json!(""), Some(false))?;
            }
            Metadata::UnreadableFile => {
                unreadable(&catalog, failed, "recordPath", "fileName")?;
            }
            Metadata::UnreadableType => {
                unreadable(&catalog, failed, "sourceType", "sourceType")?;
            }
        }
    }
    Ok(())
}

fn aliases_and_bounded_examples() -> TestResult {
    let bytes = fixture::broken_lists(12, false);
    original_aliases(&bytes)?;
    let source = Fixture::open(&bytes, Input::Plain, Vec::new())?;
    let (_, catalog) = source.diagnostic()?;
    let failed = check::rows(&catalog, "/exportHealth/failedRecords")?;
    assert_eq!(
        failed.len(),
        12,
        "exact failed identities are never sampled or merged by GUID"
    );
    let diagnostic = check::rows(&catalog, "/exportHealth/diagnostics")?
        .iter()
        .find(|row| {
            row.get("code") == Some(&json!("sourceReadFailure"))
                && row.get("kind") == Some(&json!("blueprintCategoryLists"))
        })
        .ok_or("actual aggregated read diagnostic")?;
    assert_eq!(
        diagnostic.get("count"),
        Some(&json!(12)),
        "the aggregate counts physical source failures"
    );
    let examples = check::rows(diagnostic, "/examples")?;
    assert_eq!(
        examples.len(),
        8,
        "the existing deterministic example bound stays unchanged"
    );
    for (index, row) in failed.iter().enumerate() {
        let index = u32::try_from(index)?;
        let cell = check::present(row, "recordIndex", &json!(index), Some(false))?;
        check::header(
            &catalog,
            cell,
            index,
            "recordIndex",
            "selector",
            "archiveEntry",
        )?;
    }
    for (index, example) in examples.iter().enumerate() {
        check::present(example, "recordIndex", &json!(index), Some(false))?;
    }
    check::closure(&catalog)
}

fn original_aliases(bytes: &[u8]) -> TestResult {
    let forge = DataForge::open(bytes)?;
    for index in 0..12 {
        let record = forge.record(index).ok_or("original physical alias")?;
        assert_eq!(
            record.index(),
            index,
            "physical positions remain distinct despite equal GUIDs"
        );
        assert_eq!(
            record.guid().to_string(),
            fixture::guid(fixture::LIST),
            "the original GUID alias is real"
        );
        assert!(
            record.tree().is_err(),
            "each actual layout is independently broken"
        );
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Metadata {
    EmptyFile,
    UnreadableFile,
    UnreadableType,
}
fn metadata(state: Metadata) -> TestResult<Vec<u8>> {
    let name = if matches!(state, Metadata::UnreadableType) {
        "HealthUnreadableType"
    } else {
        "Tag"
    };
    let file = if matches!(state, Metadata::EmptyFile) {
        ""
    } else {
        "synthetic/health/BadFilename.xml"
    };
    let mut bytes = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(name)
                .field("tagName", TestType::String)
                .declared_size(5),
        )
        .record(
            TestRecord::new(fixture::LIST, name, "HealthMetadata.Subject", file)
                .value("tagName", TestValue::text("StoredTag")),
        )
        .build();
    let token = match state {
        Metadata::EmptyFile => return Ok(bytes),
        Metadata::UnreadableFile => file,
        Metadata::UnreadableType => name,
    };
    let matches: Vec<_> = bytes
        .windows(token.len())
        .enumerate()
        .filter_map(|(index, value)| (value == token.as_bytes()).then_some(index))
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "the exact original metadata token is unique before corruption"
    );
    let index = *matches.first().ok_or("unique metadata token")?;
    *bytes.get_mut(index).ok_or("original metadata byte")? = 0xff;
    Ok(bytes)
}
fn original(bytes: &[u8], state: Metadata) -> TestResult {
    let forge = DataForge::open(bytes)?;
    let record = forge.record(0).ok_or("original metadata subject")?;
    assert_eq!(
        record.guid().to_string(),
        fixture::guid(fixture::LIST),
        "GUID remains nonnullable and independently readable"
    );
    assert_eq!(record.index(), 0, "physical index remains nonnullable");
    match state {
        Metadata::EmptyFile => assert_eq!(
            record.stored_file_name()?,
            "",
            "empty is a present original string"
        ),
        Metadata::UnreadableFile => assert!(
            record.stored_file_name().is_err(),
            "the original getter actually fails"
        ),
        Metadata::UnreadableType => assert!(
            record.type_name().is_err(),
            "the original type getter actually fails"
        ),
    }
    Ok(())
}
fn unreadable(catalog: &Value, row: &Value, field: &str, original: &str) -> TestResult {
    let cell = check::cell(row, field, "unreadable")?;
    assert_eq!(
        cell.get("containsEncryptedSource"),
        Some(&Value::Null),
        "a failed getter with no retained contributor has an explicit unknown marker"
    );
    assert_eq!(
        cell.get("sourceSetComplete"),
        Some(&json!(false)),
        "a failed original getter cannot certify a complete retained value"
    );
    assert!(
        check::rows(cell, "/sourceUsesRefs")?.is_empty(),
        "attempted queries are not immediate contributing references"
    );
    original_header_attempt(catalog, original)
}

fn original_header_attempt(catalog: &Value, original: &str) -> TestResult {
    let inventory = check::rows(catalog, "/provenance/uses")?;
    let attempted: Vec<_> = inventory
        .iter()
        .filter(|used| {
            used.get("role") == Some(&json!("attempted"))
                && used.get("scope") == Some(&json!("payload"))
                && used.pointer("/location/type") == Some(&json!("subresource"))
                && used.pointer("/location/namespace") == Some(&json!("dataforgeRecordHeader"))
                && used.pointer("/location/field") == Some(&json!(original))
                && used.pointer("/location/indices") == Some(&json!(["0"]))
        })
        .collect();
    assert_eq!(
        attempted.len(),
        1,
        "the failed getter retains exactly its original physical header attempt"
    );
    let used = attempted.first().ok_or("actual original header attempt")?;
    let source = check::source(catalog, used)?;
    assert_eq!(
        source.get("kind"),
        Some(&json!("archiveEntry")),
        "the attempt keeps its original Archive DCB owner"
    );
    assert_eq!(
        source.pointer("/path/value"),
        Some(&json!("Data\\Game2.dcb")),
        "no other input supplies the unreadable original Record header"
    );
    Ok(())
}

#[test]
fn unreadable_original_path_remains_a_global_candidate_in_the_public_build() -> TestResult {
    let bytes = metadata(Metadata::UnreadableFile)?;
    original(&bytes, Metadata::UnreadableFile)?;
    let source = Fixture::open(&bytes, Input::Plain, Vec::new())?;
    let (_, catalog) = source.diagnostic()?;
    let failed = check::rows(&catalog, "/exportHealth/failedRecords")?
        .iter()
        .find(|failed| {
            failed.get("kind") == Some(&Value::Null)
                && failed.get("role") == Some(&json!("selectionCandidate"))
        })
        .ok_or("actual global unknown-kind metadata candidate")?;
    assert_eq!(
        failed.get("stage"),
        Some(&json!("selection")),
        "the generic source scan retains the original failure before any Tree is required"
    );
    for kind in check::rows(&catalog, "/exportHealth/kinds")? {
        assert_eq!(
            kind.get("sourceInventoryComplete"),
            Some(&json!(false)),
            "an unreadable source path cannot prove an empty selected kind"
        );
    }
    unreadable(&catalog, failed, "recordPath", "fileName")?;
    check::closure(&catalog)
}
#[test]
fn a_filtered_original_failure_produces_no_unused_header_payload_claims() -> TestResult {
    let bytes = fixture::broken_lists(1, false);
    let source = Fixture::open(&bytes, Input::Plain, Vec::new())?;
    let forge = source.entry.open()?;
    let record = forge.record(0).ok_or("actual original broken Record")?;
    assert!(
        record.tree().is_err(),
        "the original body would fail if selected"
    );
    let filter = p4kx_extraction::Filter::new("unselected/*");
    let options = p4kx_export::CatalogOptions {
        filter: Some(&filter),
        ..fixture::options()
    };
    let built = p4kx_export::build_catalog_from(&source.archive, &source.entry, &options)?;
    assert!(
        built.summary().export_health().is_complete(),
        "ordinary filtering preserves successful absence without reading the broken body"
    );
    assert!(built.summary().source_inventory().uses().iter().all(|used| {
        used.role() != p4kx_extraction::SourceUseRole::Payload
            || !matches!(used.location(), p4kx_extraction::SourceLocation::Subresource { namespace, field, .. }
                if namespace == "dataforgeRecordHeader" && (field == "guid" || field == "recordName"))
    }), "the actual public build retains no unused GUID/name header payload claim");
    Ok(())
}
