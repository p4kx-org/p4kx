// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::starmap::{DatasetAvailability, JumpInventorySnapshot};
use p4kx_extraction::{FieldOrigin, SourceLocation, SourceValueState};
use serde_json::json;

use super::{Fixture, TestResult, open_fixture, verify_parity};

#[test]
fn external_without_a_target_cannot_certify_partner_absence() -> TestResult<()> {
    for (fixture, name_field) in [
        (Fixture::ExternalNameMissing, "$absent:@name"),
        (Fixture::ExternalNameEmpty, "@name"),
    ] {
        let (_directory, archive) = open_fixture(fixture)?;
        let query = verify_parity(&archive)?;
        assert_eq!(
            query.coverage.systems, 2,
            "the malformed beta declaration is examined with alpha"
        );
        assert_eq!(
            query.coverage.incomplete_partner_occurrences, 1,
            "only the malformed route leaves partner evidence incomplete"
        );
        assert!(
            matches!(
                query.connections.availability(),
                DatasetAvailability::Unavailable
            ),
            "a malformed target prevents global relationship absence"
        );
        let [_, _, beta] = query.endpoints.as_slice() else {
            return Err(
                "the healthy alpha occurrences and malformed beta occurrence remain".into(),
            );
        };
        let row = serde_json::to_value(beta)?;
        for pointer in [
            "/targetPath/state",
            "/targetCandidates/state",
            "/gate/state",
        ] {
            assert_eq!(
                row.pointer(pointer),
                Some(&json!("invalid")),
                "the unusable selector remains invalid at {pointer}"
            );
        }
        let partner = beta.declared_partner();
        assert_eq!(
            partner.state(),
            SourceValueState::Unreadable,
            "the unusable target cannot supply partner evidence"
        );
        assert!(
            !partner.origin().source_set_complete(),
            "the missing locator leaves partner provenance incomplete"
        );
        verify_selector_sources(&query, partner.origin(), name_field)?;
    }
    verify_local_endpoint()?;
    verify_missing_gate_member()
}

fn verify_selector_sources(
    query: &JumpInventorySnapshot,
    origin: &FieldOrigin,
    name_field: &str,
) -> TestResult<()> {
    let fields = origin
        .source_uses_refs()
        .iter()
        .map(|reference| {
            let used = query
                .source_inventory
                .iter_uses()
                .find(|used| used.id() == reference)
                .ok_or("partner evidence resolves through the query inventory")?;
            assert!(
                query
                    .source_inventory
                    .iter_sources()
                    .any(|source| source.id() == used.source()),
                "the selector observation names an original physical source"
            );
            Ok(used.location())
        })
        .collect::<TestResult<Vec<_>>>()?;
    for field_name in ["@external", name_field] {
        assert!(fields.iter().any(|location| {
            matches!(location, SourceLocation::Subresource { namespace, field, .. } if namespace == "xml" && field == field_name)
        }), "partner evidence retains the exact authored XML location {field_name}");
    }
    Ok(())
}

fn last_endpoint(query: &JumpInventorySnapshot) -> TestResult<serde_json::Value> {
    Ok(serde_json::to_value(
        query.endpoints.last().ok_or("beta endpoint")?,
    )?)
}

fn verify_local_endpoint() -> TestResult<()> {
    let (_directory, archive) = open_fixture(Fixture::LocalEndpoint)?;
    let query = verify_parity(&archive)?;
    assert!(
        matches!(
            query.connections.availability(),
            DatasetAvailability::Absent
        ),
        "a local endpoint allows confirmed relationship absence"
    );
    let beta = last_endpoint(&query)?;
    assert_eq!(
        beta.pointer("/gate/state"),
        Some(&json!("absent")),
        "the local endpoint has no external gate declaration"
    );
    assert_eq!(
        beta.pointer("/declaredPartner/state"),
        Some(&json!("absent")),
        "the original local declaration has no partner"
    );
    Ok(())
}

fn verify_missing_gate_member() -> TestResult<()> {
    let (_directory, archive) = open_fixture(Fixture::MissingMetadata)?;
    let query = verify_parity(&archive)?;
    assert!(
        matches!(
            query.connections.availability(),
            DatasetAvailability::Unavailable
        ),
        "missing metadata leaves the relationship scope unknown"
    );
    let beta = last_endpoint(&query)?;
    assert_eq!(
        beta.pointer("/gate/value/metadata/state"),
        Some(&json!("absent")),
        "the complete gate directory confirms the metadata member is absent"
    );
    assert_eq!(
        beta.pointer("/declaredPartner/state"),
        Some(&json!("unreadable")),
        "an absent member leaves partner evidence unreadable"
    );
    Ok(())
}
