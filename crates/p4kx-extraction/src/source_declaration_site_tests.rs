// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::mem::size_of_val;
use std::ptr;

use super::{Coordinates, DataForgeSite, RetainedLocation, StoredDataForgeSite};
use crate::{
    InputScope, SourceEvidenceError, SourceInput, SourceLocation, SourceUse, SourceUseRole,
    SourceUses,
};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn declaration_sites_use_inline_positions_and_static_names() -> TestResult {
    assert_eq!(
        size_of::<Coordinates>(),
        size_of::<([u64; 2], u64)>(),
        "the declarations use the existing two-coordinate and aligned tag footprint",
    );
    assert_eq!(
        size_of::<DataForgeSite>(),
        size_of::<Coordinates>() + size_of::<&'static str>(),
        "a site holds only inline coordinates and a borrowed field label",
    );
    assert!(
        size_of::<StoredDataForgeSite>() <= size_of::<SourceLocation>(),
        "the specialized variant fits the existing retained-location footprint",
    );

    let registry = SourceUses::with_compact_coordinates();
    let scope = registry.observe(SourceInput::Unknown, InputScope::Payload)?;
    for (location, payload_bytes) in [
        (survey("dataforgeValue", &[10, u64::MAX]), 37),
        (
            survey("dataforgeRecordDeclarations", &[u64::from(u32::MAX)]),
            42,
        ),
    ] {
        let SourceLocation::Subresource {
            namespace,
            indices,
            field,
        } = &location
        else {
            return Err("an original declaration subresource is expected".into());
        };
        assert_eq!(
            namespace.len() + size_of_val(indices.as_slice()) + field.len(),
            payload_bytes,
            "the specialized row retains none of these separately allocated payload bytes",
        );
        let used = scope.at(location.clone(), SourceUseRole::Selector)?;
        assert_location_without_materialization(&used, &location)?;
    }
    for location in [
        survey("dataforgeRecordDeclarations", &[1, 2]),
        survey("dataforgeRecordDeclarations", &[u64::MAX]),
        subresource("dataforgeRecordDeclarations", &[1], "$unknown"),
    ] {
        let used = scope.at(location.clone(), SourceUseRole::Selector)?;
        assert!(
            matches!(used.description().location, RetainedLocation::Original(_)),
            "foreign coordinate shapes keep the original owned representation"
        );
        assert_eq!(
            used.description().location(),
            &location,
            "exact fallback coordinate"
        );
    }
    Ok(())
}

#[test]
fn declaration_sites_keep_lexical_ids_exact_duplicate_and_foreign_authority() -> TestResult {
    let locations = [
        survey("dataforgeRecordDeclarations", &[2]),
        survey("dataforgeValue", &[2, 1]),
        survey("dataforgeRecordDeclarations", &[10]),
        survey("dataforgeValue", &[10, 1]),
    ];
    let registry = SourceUses::with_compact_coordinates();
    let scope = registry.observe(SourceInput::Unknown, InputScope::Payload)?;
    let peer = SourceUses::with_compact_coordinates();
    let foreign_scope = peer.observe(SourceInput::Unknown, InputScope::Payload)?;
    for location in locations {
        let actual = scope.at(location.clone(), SourceUseRole::Payload)?;
        let duplicate = scope.at(location.clone(), SourceUseRole::Payload)?;
        assert!(
            ptr::eq(actual.description(), duplicate.description()),
            "repeated registration preserves the exact description authority"
        );
        let foreign = foreign_scope.at(location.clone(), SourceUseRole::Payload)?;
        assert_eq!(
            foreign.reference(),
            actual.reference(),
            "foreign IDs are text-identical"
        );
        assert!(
            matches!(
                scope.at_observed_location(&foreign, SourceUseRole::Selector),
                Err(SourceEvidenceError::ForeignEvidence)
            ),
            "wire identity cannot restore foreign authority"
        );
        let selector = scope.at_observed_location(&actual, SourceUseRole::Selector)?;
        assert_location_without_materialization(&selector, &location)?;
    }
    let snapshot = registry.freeze()?;
    let ids: Vec<_> = snapshot.iter_uses().map(|row| row.id().as_str()).collect();
    assert_eq!(
        ids,
        [
            "c2:u0pps?27:dataforgeRecordDeclarations10.:$fields",
            "c2:u0pps?27:dataforgeRecordDeclarations2.:$fields",
            "c2:u0ppsv10.1.:$fields",
            "c2:u0ppsv2.1.:$fields",
            "c2:u0pss?27:dataforgeRecordDeclarations10.:$fields",
            "c2:u0pss?27:dataforgeRecordDeclarations2.:$fields",
            "c2:u0pssv10.1.:$fields",
            "c2:u0pssv2.1.:$fields",
        ],
        "IDs, lexical order and deduplication remain exact"
    );
    Ok(())
}

fn assert_location_without_materialization(
    use_: &SourceUse,
    location: &SourceLocation,
) -> TestResult {
    let description = use_.description();
    assert!(
        matches!(description.location, RetainedLocation::DataForge(_)),
        "exact declaration uses compact storage"
    );
    let typed = serde_json::to_vec(location)?;
    let borrowed = serde_json::to_vec(&description.location)?;
    assert_eq!(borrowed, typed, "the wire coordinate is byte-identical");
    assert_eq!(
        description.location.compact_is_materialized(),
        Some(false),
        "normal serialization does not allocate an owned view"
    );
    assert_eq!(
        description.location(),
        location,
        "external callers can still borrow the original shape"
    );
    Ok(())
}

fn survey(namespace: &str, indices: &[u64]) -> SourceLocation {
    subresource(namespace, indices, "$fields")
}

fn subresource(namespace: &str, indices: &[u64], field: &str) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: namespace.to_owned(),
        indices: indices.to_vec(),
        field: field.to_owned(),
    }
}
