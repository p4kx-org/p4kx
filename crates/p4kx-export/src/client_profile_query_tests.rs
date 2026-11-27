// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::OpenOptions;
use std::io::Write;
use std::time::UNIX_EPOCH;

use p4kx_extraction::{CancelToken, Filter, InputScope, ProvenanceMode, SourceUseRole};
use p4kx_testkit::{TestEntry, test_key};
use serde_json::{Value, json};

use super::{GENERAL, GENERAL_XML, TestResult, VR, archive, at, catalog};
use crate::{
    CatalogError, CatalogOptions, ProfileKind, ProfileState, build_catalog,
    read_client_profile_settings, read_client_profile_settings_for,
    read_client_profile_settings_for_with_provenance_mode,
    read_client_profile_settings_with_provenance_mode,
};

const PROFILES: &str = "/configurations/clientProfileSettings";

#[test]
fn typed_profile_queries_match_full_and_filtered_catalog_rows() -> TestResult {
    let (_directory, archive) = archive(
        vec![
            catalog::dataforge(),
            TestEntry::stored(GENERAL, GENERAL_XML),
            TestEntry::stored(VR, GENERAL_XML).encrypted(),
        ],
        Some(test_key()),
    )?;
    let filter = Filter::parse(VR)?;
    let complete = CatalogOptions::new(UNIX_EPOCH);
    let filtered = CatalogOptions {
        filter: Some(&filter),
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let full =
        read_client_profile_settings_with_provenance_mode(&archive, ProvenanceMode::Full, None)?;
    let vr = read_client_profile_settings_for_with_provenance_mode(
        &archive,
        ProfileKind::VirtualReality,
        ProvenanceMode::Full,
        None,
    )?;
    for (options, report) in [(&complete, &full), (&filtered, &vr)] {
        let built = build_catalog(&archive, options.with_full_provenance())?;
        let wire: Value = serde_json::from_slice(built.json())?;
        assert_eq!(
            &serde_json::to_value(report.settings())?,
            at(&wire, PROFILES)?,
            "values and immediate source uses match the canonical projection"
        );
    }

    let general = vr.settings().document(ProfileKind::General);
    assert_eq!(general.status().value(), Some(&ProfileState::Filtered));
    assert!(
        general.elements().is_empty(),
        "the sibling payload was excluded"
    );
    let document = vr.settings().document(ProfileKind::VirtualReality);
    assert_eq!(document.status().value(), Some(&ProfileState::Read));
    assert_eq!(document.requested_path(), VR);
    let [candidate] = document.candidates() else {
        return Err("one physical VR document candidate".into());
    };
    assert_eq!(
        candidate.source_path().value().map(String::as_str),
        Some(VR)
    );
    assert!(
        vr.provenance()
            .iter_uses()
            .any(|row| row.source() == candidate.input_ref() && row.scope() == InputScope::Payload),
        "the selected encrypted payload has scoped origin evidence"
    );
    Ok(())
}

#[test]
fn narrow_profile_query_retains_typed_values_without_opening_other_payloads() -> TestResult {
    let (_directory, archive) = archive(
        vec![
            TestEntry::stored(GENERAL, GENERAL_XML),
            TestEntry::stored(VR, "<Attributes/>").encrypted(),
        ],
        None,
    )?;
    let report = read_client_profile_settings_for_with_provenance_mode(
        &archive,
        ProfileKind::General,
        ProvenanceMode::Full,
        None,
    )?;
    assert_eq!(
        report.build(),
        archive.build(),
        "read the Build from the Archive"
    );
    assert_eq!(report.settings().decoded_documents(), 1);
    assert!(!report.settings().has_failed_inputs());

    let general = report.settings().document(ProfileKind::General);
    let attribute = general.elements().get(1).ok_or("first Attr element")?;
    assert_eq!(attribute.parent().value(), Some(&Some(0)));
    assert_eq!(attribute.name().value().map(String::as_str), Some("Attr"));
    let declaration = attribute.declarations().get(1).ok_or("value declaration")?;
    let text = declaration.value();
    assert_eq!(text.value().value().map(String::as_str), Some(" 001&2 "));
    let lexeme = text.lexeme().ok_or("distinct authored spelling")?;
    assert!(lexeme.value().is_some_and(|value| value.contains("&amp;")));
    for id in text.value().origin().source_uses_refs() {
        assert!(
            report.provenance().iter_uses().any(|row| row.id() == id),
            "each direct source-use reference closes in the result"
        );
    }

    let sibling = report.settings().document(ProfileKind::VirtualReality);
    assert_eq!(sibling.status().value(), Some(&ProfileState::Filtered));
    let [candidate] = sibling.candidates() else {
        return Err("directory retained the unselected VR candidate".into());
    };
    assert!(
        !report.provenance().iter_uses().any(|row| {
            row.source() == candidate.input_ref()
                && (row.scope() == InputScope::Payload || row.role() == SourceUseRole::Attempted)
        }),
        "the encrypted unreadable sibling contributes only directory evidence"
    );
    Ok(())
}

#[test]
fn profile_queries_default_to_an_encrypted_value_without_proof_inventory() -> TestResult {
    let (_directory, archive) = archive(
        vec![TestEntry::stored(VR, GENERAL_XML).encrypted()],
        Some(test_key()),
    )?;
    let value_path = "/settings/documents/1/elements/1/declarations/1/value/value";
    for report in [
        read_client_profile_settings(&archive, None)?,
        read_client_profile_settings_for(&archive, ProfileKind::VirtualReality, None)?,
    ] {
        let encoded = serde_json::to_value(&report)?;
        assert_eq!(
            at(&encoded, value_path)?,
            &json!({"state": "present", "value": " 001&2 ", "containsEncryptedSource": true}),
            "both defaults retain the authored value and encryption without proof references"
        );
        assert_eq!(
            at(&encoded, "/provenance")?,
            &json!({"sources": [], "uses": []}),
            "marker results retain no proof inventory"
        );
    }
    let full = read_client_profile_settings_for_with_provenance_mode(
        &archive,
        ProfileKind::VirtualReality,
        ProvenanceMode::Full,
        None,
    )?;
    let encoded = serde_json::to_value(&full)?;
    let field = at(&encoded, value_path)?;
    assert_eq!(
        (field.get("value"), field.get("containsEncryptedSource")),
        (Some(&json!(" 001&2 ")), Some(&json!(true))),
        "full provenance retains the same authored value and encryption"
    );
    assert_eq!(
        field.get("sourceSetComplete"),
        Some(&json!(true)),
        "the encrypted declaration has complete direct evidence"
    );
    let references = super::rows(field, "/sourceUsesRefs")?;
    let [reference] = references else {
        return Err("expected one source use for the original value".into());
    };
    assert!(
        full.provenance().iter_uses().any(|used| {
            Some(used.id().as_str()) == reference.as_str() && used.scope() == InputScope::Payload
        }),
        "full provenance resolves the original payload use in its own inventory"
    );
    Ok(())
}

#[test]
fn scoped_profiles_reject_cancellation_and_stale_archive() -> TestResult {
    let (directory, archive) = archive(vec![TestEntry::stored(GENERAL, GENERAL_XML)], None)?;
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(matches!(
        read_client_profile_settings_for(&archive, ProfileKind::General, Some(&cancel)),
        Err(CatalogError::Cancelled)
    ));
    let mut file = OpenOptions::new()
        .append(true)
        .open(directory.path().join("Data.p4k"))?;
    file.write_all(b"changed")?;
    drop(file);
    assert!(matches!(
        read_client_profile_settings_for(&archive, ProfileKind::General, None),
        Err(CatalogError::ArchiveChanged(_))
    ));
    Ok(())
}
