// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    DefinitionStatus, LocationName, MissionLocationTemplate, MissionLocationTemplateSnapshot,
    SourceRecordReference,
};
use p4kx_extraction::{SourceLocation, SourceUseRole, SourceValueState, SourcedValue};
use serde_json::Value;

use super::fixture::{self, Case};
use crate::commodity_v3_fixture::{self as common, TestResult};

#[path = "mission_location_source_marker.rs"]
mod marker_policy;

pub(super) fn verify(
    snapshot: &MissionLocationTemplateSnapshot<'_>,
    case: Case,
    marker: Option<bool>,
) -> TestResult {
    let [row] = snapshot.rows.as_slice() else {
        return Err("one selected original location".into());
    };
    let wire = serde_json::to_value(row)?;
    cells(&wire, marker, snapshot, case, "")?;
    header_source(row, snapshot);
    cluster_source(row, snapshot);
    assert_eq!(
        row.source().guid().value().copied(),
        Some(row.guid()),
        "same original GUID"
    );
    assert_eq!(
        row.source().record_index().value().copied(),
        Some(row.record_index()),
        "same original physical identity"
    );
    assert!(
        wire.get("entityClusterMemberGuid").is_none(),
        "no duplicate nullable legacy cluster view"
    );
    let expected = match case {
        Case::Sparse => SourceValueState::Absent,
        Case::NullData => SourceValueState::Null,
        Case::InvalidFlag => SourceValueState::Invalid,
        _ => SourceValueState::Present,
    };
    assert_eq!(row.disabled().state(), expected, "original Bool state");
    if matches!(case, Case::Present) {
        assert_eq!(row.disabled().value(), Some(&false), "false is present");
    }
    reference(row.entity_cluster_member(), case);
    names(row.names(), case)
}

fn reference(cell: &SourcedValue<Box<SourceRecordReference<'_>>>, case: Case) {
    let Some(reference) = cell.value() else {
        assert!(
            matches!(case, Case::Sparse | Case::NullData),
            "only absent/null original owners omit the reference"
        );
        return;
    };
    let expected_guid = if matches!(case, Case::Dangling) {
        fixture::MISSING
    } else {
        fixture::TARGET
    };
    assert_eq!(
        reference.guid().value().map(ToString::to_string),
        Some(common::guid(expected_guid)),
        "the original authored GUID survives target failure"
    );
    match case {
        Case::Dangling => assert!(
            matches!(reference.status(), DefinitionStatus::Dangling),
            "original dangling lookup"
        ),
        Case::Unexpected => assert!(
            matches!(reference.status(), DefinitionStatus::UnexpectedTargetType),
            "original incompatible target"
        ),
        _ => assert!(
            matches!(reference.status(), DefinitionStatus::Present),
            "original compatible target"
        ),
    }
    if matches!(case, Case::Dangling) {
        assert!(
            reference.target_record_index().value().is_none(),
            "no invented physical target"
        );
        assert!(
            reference.record_path().value().is_none(),
            "no invented target path"
        );
    }
}

fn names(cell: &SourcedValue<Vec<SourcedValue<LocationName>>>, case: Case) -> TestResult {
    let Some(rows) = cell.value() else {
        assert!(
            matches!(case, Case::Sparse | Case::NullNames | Case::NullData),
            "collection absence follows the original owner"
        );
        return Ok(());
    };
    if matches!(case, Case::EmptyNames) {
        assert!(rows.is_empty(), "empty is a present authored array");
        return Ok(());
    }
    if matches!(case, Case::NameSlots) {
        assert_eq!(rows.len(), 4, "all original slot positions survive");
        assert_eq!(
            rows.get(1).ok_or("null slot")?.state(),
            SourceValueState::Null,
            "authored null slot"
        );
        assert_eq!(
            rows.get(3).ok_or("unreadable slot")?.state(),
            SourceValueState::Unreadable,
            "unreadable original pointer slot"
        );
    } else {
        assert_eq!(rows.len(), 2, "both repeated original names survive");
    }
    repeated_names(rows, case)
}

fn repeated_names(rows: &[SourcedValue<LocationName>], case: Case) -> TestResult {
    let first = rows
        .first()
        .and_then(SourcedValue::value)
        .ok_or("first original name")?;
    let repeated = rows
        .get(if matches!(case, Case::NameSlots) {
            2
        } else {
            1
        })
        .and_then(SourcedValue::value)
        .ok_or("repeated original name")?;
    assert_eq!(
        first.display_name.value(),
        repeated.display_name.value(),
        "same key uses one protected text identity"
    );
    assert_ne!(
        first.display_name.origin().source_uses_refs(),
        repeated.display_name.origin().source_uses_refs(),
        "repeated physical text operands retain distinct observations"
    );
    Ok(())
}

fn cells(
    value: &Value,
    marker: Option<bool>,
    snapshot: &MissionLocationTemplateSnapshot<'_>,
    case: Case,
    path: &str,
) -> TestResult {
    match value {
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                cells(value, marker, snapshot, case, &format!("{path}/{index}"))?;
            }
        }
        Value::Object(object) => {
            if object.contains_key("state") && object.contains_key("sourceUsesRefs") {
                marker_policy::verify(object, marker, case, path)?;
                for used in object
                    .get("sourceUsesRefs")
                    .and_then(Value::as_array)
                    .ok_or("immediate source references")?
                {
                    assert!(
                        snapshot
                            .source_inventory
                            .iter_uses()
                            .any(|candidate| Some(candidate.id().as_str()) == used.as_str()),
                        "case={case:?} pointer={path}: datum references the same original source inventory"
                    );
                }
            }
            for (name, value) in object {
                let name = name.replace('~', "~0").replace('/', "~1");
                cells(value, marker, snapshot, case, &format!("{path}/{name}"))?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn header_source(
    row: &MissionLocationTemplate<'_>,
    snapshot: &MissionLocationTemplateSnapshot<'_>,
) {
    let guid = row.source().guid();
    assert!(snapshot.source_inventory.iter_uses().any(|used| {
        guid.origin().source_uses_refs().contains(used.id())
            && used.role() == SourceUseRole::Payload
            && matches!(used.location(), SourceLocation::Subresource { namespace, field, indices }
                if namespace == "dataforgeRecordHeader" && field == "guid"
                    && indices.as_slice() == [u64::from(row.record_index())])
    }), "the GUID comes from this physical original header, independently of the body");
}

fn cluster_source(
    row: &MissionLocationTemplate<'_>,
    snapshot: &MissionLocationTemplateSnapshot<'_>,
) {
    let Some(link) = row.entity_cluster_member().value() else {
        return;
    };
    let guid = link.guid();
    assert!(snapshot.source_inventory.iter_uses().any(|used| {
        guid.origin().source_uses_refs().contains(used.id())
            && used.role() == SourceUseRole::Payload
            && matches!(used.location(), SourceLocation::Subresource { namespace, field, indices }
                if namespace == "dataforgeValue" && matches!(field.as_str(), "$value" | "$error.guid")
                    && indices.first() == Some(&u64::from(row.record_index())))
    }), "the stored cluster GUID retains its original body/error operand, not a copied target GUID");
}
