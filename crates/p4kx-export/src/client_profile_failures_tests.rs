// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::CancelToken;
use p4kx_testkit::TestEntry;

use super::{
    GENERAL, GENERAL_SOURCE, TestResult, VR, archive, at, immediate_use, observe, rows, state,
};
use crate::{CatalogError, read_client_profile_settings};

const VALID_XML: &str = "<Attributes><Attr name='fixture_setting' value='1'/></Attributes>";

#[test]
fn a_missing_profile_has_complete_absence_while_the_other_is_read() -> TestResult {
    let report = observe(vec![TestEntry::stored(GENERAL, VALID_XML)], None)?;
    assert_eq!(
        report.settings().decoded_documents(),
        1,
        "General is readable"
    );
    assert!(
        !report.settings().has_failed_inputs(),
        "the optional missing document does not fail"
    );
    let document = serde_json::to_value(report)?;
    let vr = at(&document, "/settings/documents/1")?;
    state(vr, "missing", true)?;
    assert!(
        rows(vr, "/candidates")?.is_empty(),
        "the missing document has no candidates"
    );
    Ok(())
}

#[test]
fn unreadable_and_malformed_profiles_keep_their_own_failure_and_candidate() -> TestResult {
    let report = observe(
        vec![
            TestEntry::stored(GENERAL, VALID_XML).encrypted(),
            TestEntry::stored(VR, "not an XML document"),
        ],
        None,
    )?;
    assert!(report.settings().has_failed_inputs(), "both inputs failed");
    assert_eq!(
        report.settings().decoded_documents(),
        0,
        "no decoded documents"
    );
    let document = serde_json::to_value(report)?;
    let [general, vr] = rows(&document, "/settings/documents")? else {
        return Err("both failed decisions".into());
    };
    for (profile, expected, complete, role) in [
        (general, "unreadable", false, "attempted"),
        (vr, "malformed", true, "conversion"),
    ] {
        state(profile, expected, complete)?;
        let [entry] = rows(profile, "/candidates")? else {
            return Err("the failed candidate still has physical identity".into());
        };
        let status = at(profile, "/status")?;
        let source = at(entry, "/inputRef")?;
        let observed = if complete {
            immediate_use(&document, status)?
        } else {
            assert_eq!(
                rows(status, "/sourceUsesRefs")?,
                &[] as &[serde_json::Value]
            );
            assert!(at(status, "/containsEncryptedSource")?.is_null());
            rows(&document, "/provenance/uses")?
                .iter()
                .find(|usage| {
                    usage.get("source") == Some(source)
                        && usage.get("role").and_then(serde_json::Value::as_str) == Some(role)
                })
                .ok_or("the inventory records the failed read attempt")?
        };
        assert_eq!(
            at(observed, "/source")?,
            at(entry, "/inputRef")?,
            "the failure identifies its own input"
        );
        assert_eq!(
            at(observed, "/role")?,
            role,
            "read and parse failures have different evidence"
        );
    }
    Ok(())
}

#[test]
fn duplicate_physical_paths_are_ambiguous_without_suppressing_the_other_document() -> TestResult {
    let report = observe(
        vec![
            TestEntry::stored(GENERAL, VALID_XML),
            TestEntry::stored(GENERAL_SOURCE, VALID_XML),
            TestEntry::stored(VR, VALID_XML),
        ],
        None,
    )?;
    assert!(
        report.settings().has_failed_inputs(),
        "the General candidate is ambiguous"
    );
    assert_eq!(
        report.settings().decoded_documents(),
        1,
        "the unambiguous VR document survives"
    );
    let document = serde_json::to_value(report)?;
    let [general, vr] = rows(&document, "/settings/documents")? else {
        return Err("both independent decisions".into());
    };
    state(general, "ambiguous", true)?;
    state(vr, "read", true)?;
    let [first, second] = rows(general, "/candidates")? else {
        return Err("both competing physical candidates".into());
    };
    assert_eq!(
        at(first, "/sourcePath/value")?,
        GENERAL,
        "first original spelling"
    );
    assert_eq!(
        at(second, "/sourcePath/value")?,
        GENERAL_SOURCE,
        "second original spelling"
    );
    assert_ne!(
        at(first, "/entryIndex")?,
        at(second, "/entryIndex")?,
        "distinct directory positions"
    );
    assert_ne!(
        at(first, "/inputRef")?,
        at(second, "/inputRef")?,
        "distinct physical source IDs"
    );
    Ok(())
}

#[test]
fn a_cancelled_profile_read_returns_an_error() -> TestResult {
    let (_directory, archive) = archive(vec![TestEntry::stored(GENERAL, VALID_XML)], None)?;
    let token = CancelToken::new();
    token.cancel();
    assert!(
        matches!(
            read_client_profile_settings(&archive, Some(&token)),
            Err(CatalogError::Cancelled)
        ),
        "a caller cancellation aborts the report"
    );
    Ok(())
}
