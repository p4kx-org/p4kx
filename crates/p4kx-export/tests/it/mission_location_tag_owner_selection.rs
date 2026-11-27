// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, MissionLocationTemplateSnapshot,
};
use p4kx_extraction::{SourceLocation, SourceUseRole};
use serde_json::Value;

#[path = "mission_location_tag_owner_fixture.rs"]
mod fixture;

use super::super::TestResult;
use crate::commodity_v3_fixture::{self, DcbOrigin, guid};
use crate::spawns_fixture::LOCATION_A;
use fixture::Ancestor;

struct FailedSelectionProof {
    attempt: Value,
    selector: Value,
}

#[test]
fn mission_location_tag_descendants_retain_failed_ancestor_selection() -> TestResult {
    for ancestor in Ancestor::ALL {
        let (_folder, archive) =
            commodity_v3_fixture::open(&fixture::database(ancestor), DcbOrigin::Plain)?;
        full_proof::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?
            .with_mission_location_templates(
                CatalogRecordSelection::Guid(guid(LOCATION_A).parse()?),
                None,
                |snapshot| verify_snapshot(&snapshot, ancestor),
            )??;
    }
    Ok(())
}

fn verify_snapshot(
    snapshot: &MissionLocationTemplateSnapshot<'_>,
    ancestor: Ancestor,
) -> TestResult {
    let [row] = snapshot.rows.as_slice() else {
        return Err("the first readable legacy ancestor yields one template".into());
    };
    let row = serde_json::to_value(row)?;
    let selected = failed_selection(snapshot)?;
    if matches!(ancestor, Ancestor::LocationData) {
        verify_location_cells(&row, &selected)?;
    }
    for path in affected_lists(ancestor) {
        verify_list(row.pointer(path).ok_or("selected tag list")?, &selected)?;
    }
    for name in ["producesTags", "consumesTags"] {
        if matches!(ancestor, Ancestor::LocationData) || name == ancestor.member() {
            verify_incomplete(row.get(name).ok_or("selected tag term")?, &selected);
            let empty = row
                .pointer(&format!("/{name}/value/negativeTags"))
                .ok_or("the empty negative operand list")?;
            verify_incomplete(empty, &selected);
            assert_eq!(
                empty.get("value"),
                Some(&Value::Array(Vec::new())),
                "an empty descendant list retains its failed ancestor proof"
            );
        }
    }
    if matches!(ancestor, Ancestor::LocationData | Ancestor::StringVariants) {
        verify_names(&row, &selected)?;
    } else {
        assert_eq!(
            row.pointer("/names/value/0/value/tag/path/sourceSetComplete"),
            Some(&Value::Bool(true)),
            "an unrelated name variant inherits no failed tag-parent attempt"
        );
    }
    Ok(())
}

fn failed_selection(
    snapshot: &MissionLocationTemplateSnapshot<'_>,
) -> Result<FailedSelectionProof, Box<dyn std::error::Error>> {
    let attempts: Vec<_> = snapshot
        .source_inventory
        .iter_uses()
        .filter(|used| used.role() == SourceUseRole::Attempted)
        .filter(|used| {
            matches!(used.location(),
            SourceLocation::Subresource { namespace, field, indices }
                if namespace == "dataforgeValue" && field == "$value" && indices.len() == 2)
        })
        .collect();
    let [attempt] = attempts.as_slice() else {
        return Err("exactly one failed ancestor value selection".into());
    };
    let selectors: Vec<_> = snapshot
        .source_inventory
        .iter_uses()
        .filter(|used| used.role() == SourceUseRole::Selector)
        .filter(|used| {
            used.source() == attempt.source()
                && used.scope() == attempt.scope()
                && used.location() == attempt.location()
        })
        .collect();
    let [selector] = selectors.as_slice() else {
        return Err("the failed value has exactly one actual selection observation".into());
    };
    assert_ne!(
        selector.id(),
        attempt.id(),
        "the attempted read and actual skip decision have separate roles"
    );
    Ok(FailedSelectionProof {
        attempt: serde_json::to_value(attempt.id())?,
        selector: serde_json::to_value(selector.id())?,
    })
}

fn verify_list(list: &Value, selected: &FailedSelectionProof) -> TestResult {
    verify_incomplete(list, selected);
    let items = list
        .get("value")
        .and_then(Value::as_array)
        .ok_or("all selected legacy tag operands")?;
    let [first, null, dangling] = items.as_slice() else {
        return Err("known, null and dangling slots are retained".into());
    };
    assert_eq!(
        first.pointer("/path/value"),
        Some(&Value::from("Leaf")),
        "the same first readable owner supplies the path"
    );
    assert_eq!(
        null.pointer("/path/state"),
        Some(&Value::from("null")),
        "the null original operand stays null"
    );
    for item in items {
        verify_incomplete(
            item.get("path").ok_or("each slot has its own path cell")?,
            selected,
        );
    }
    verify_incomplete(
        dangling
            .get("danglingGuid")
            .ok_or("dangling original GUID")?,
        selected,
    );
    Ok(())
}

const fn affected_lists(ancestor: Ancestor) -> &'static [&'static str] {
    match ancestor {
        Ancestor::LocationData => &[
            "/tags",
            "/producesTags/value/positiveTags",
            "/consumesTags/value/positiveTags",
            "/aiSpawnTags",
        ],
        Ancestor::GeneralTags => &["/tags"],
        Ancestor::ProducesTags => &["/producesTags/value/positiveTags"],
        Ancestor::ConsumesTags => &["/consumesTags/value/positiveTags"],
        Ancestor::AiSpawnTags => &["/aiSpawnTags"],
        Ancestor::StringVariants => &[],
    }
}

fn verify_names(row: &Value, selected: &FailedSelectionProof) -> TestResult {
    let names = row
        .pointer("/names/value")
        .and_then(Value::as_array)
        .ok_or("the selected string variants")?;
    assert_eq!(
        names.len(),
        3,
        "all original variant tag slots remain ordered"
    );
    verify_incomplete(
        row.get("names").ok_or("original name collection")?,
        selected,
    );
    for name in names {
        verify_incomplete(name, selected);
        verify_incomplete(
            name.pointer("/value/displayName")
                .ok_or("original protected name")?,
            selected,
        );
        verify_incomplete(
            name.pointer("/value/tag/path")
                .ok_or("the variant's tag path")?,
            selected,
        );
    }
    verify_incomplete(
        names
            .last()
            .and_then(|name| name.pointer("/value/tag/danglingGuid"))
            .ok_or("the dangling variant tag has an original GUID")?,
        selected,
    );
    Ok(())
}

fn verify_incomplete(cell: &Value, selected: &FailedSelectionProof) {
    assert_eq!(
        cell.get("sourceSetComplete"),
        Some(&Value::Bool(false)),
        "an unreadable earlier owner leaves the selected descendant incomplete"
    );
    assert_eq!(
        cell.get("containsEncryptedSource"),
        Some(&Value::Null),
        "the incomplete plain selection has no proven encryption classification"
    );
    assert!(
        cell.get("sourceUsesRefs")
            .and_then(Value::as_array)
            .is_some_and(|references| references.contains(&selected.selector)),
        "the descendant retains the actual selector of the failed ancestor"
    );
    assert!(
        cell.get("sourceUsesRefs")
            .and_then(Value::as_array)
            .is_some_and(|references| !references.contains(&selected.attempt)),
        "the attempted read stays in the inventory independently of field contributors"
    );
}

fn verify_location_cells(row: &Value, selected: &FailedSelectionProof) -> TestResult {
    for path in [
        "/disabled",
        "/isSecurityNetworkHost",
        "/entityClusterMember",
        "/entityClusterMember/value/guid",
        "/entityClusterMember/value/targetRecordIndex",
        "/entityClusterMember/value/recordPath",
        "/entityClusterMember/value/targetType",
    ] {
        verify_incomplete(
            row.pointer(path).ok_or("selected original location cell")?,
            selected,
        );
    }
    assert_eq!(
        row.pointer("/disabled/value"),
        Some(&Value::Bool(false)),
        "the first readable owner still supplies false"
    );
    assert_eq!(
        row.pointer("/entityClusterMember/value/status"),
        Some(&Value::from("present")),
        "the same original compatible target is retained"
    );
    Ok(())
}
