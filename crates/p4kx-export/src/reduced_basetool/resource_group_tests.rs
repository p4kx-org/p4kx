// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{ArchiveBuilder, GlobalIniBuilder, TestEntry, test_key};
use serde_json::Value;

use super::ReducedError;
use super::record_targets::RecordTargets;
use super::resource_groups;
use super::selected_texts::SelectedTexts;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

#[path = "../../tests/it/resource_group_records.rs"]
mod fixture;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn resource_groups_keep_physical_links_order_and_only_selected_names() -> TestResult {
    with_queries(|queries, scratch| {
        let resources =
            queries.with_resource_types(CatalogRecordSelection::All, None, |snapshot| {
                let mut targets = RecordTargets::default();
                for row in snapshot.rows() {
                    targets.insert("definitions.resourceTypes", row.source())?;
                }
                Ok::<_, ReducedError>(targets)
            })??;
        let mut texts = SelectedTexts::default();
        let groups = resource_groups::collect(queries, scratch, None, &resources, &mut texts)?;
        let definitions = serde_json::to_value(groups.definitions)?;
        let members = serde_json::to_value(groups.members)?;
        let parents = serde_json::to_value(groups.parents)?;

        let definition = row(&definitions, 0)?;
        assert!(definition.get("description").is_none());
        assert!(definition.get("defaultThumbnailPathSVG").is_none());
        let membership = row(&members, 0)?;
        assert_child_links(membership)?;
        assert_resource_links(membership)?;
        assert_parent_links(&parents)?;
        let texts = serde_json::to_value(texts)?;
        let name = texts
            .pointer("/publicNames/0")
            .ok_or("selected public name")?;
        assert_eq!(definition.pointer("/name/value"), name.pointer("/id/value"));
        assert_eq!(
            texts
                .get("gameTexts")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(0)
        );
        Ok(())
    })
}

fn assert_child_links(membership: &Value) -> TestResult {
    let children = membership
        .pointer("/groups/value")
        .and_then(Value::as_array)
        .ok_or("child slots")?;
    let [empty, wrong_type, dangling, child, repeated_child, _self] = children.as_slice() else {
        return Err("six ordered child slots".into());
    };
    assert_eq!(
        empty.pointer("/state").and_then(Value::as_str),
        Some("null")
    );
    assert_eq!(
        wrong_type.pointer("/value/status").and_then(Value::as_str),
        Some("present"),
        "the source link resolves to the actual Record independently of reduced membership"
    );
    assert_eq!(
        wrong_type
            .pointer("/value/targetType/value")
            .and_then(Value::as_str),
        Some("Other")
    );
    assert_eq!(
        wrong_type
            .pointer("/value/targetRecordIndex/value")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        dangling.pointer("/value/status").and_then(Value::as_str),
        Some("dangling")
    );
    assert_eq!(child, repeated_child);

    Ok(())
}

fn assert_resource_links(membership: &Value) -> TestResult {
    let resources = membership
        .pointer("/resources/value")
        .and_then(Value::as_array)
        .ok_or("resource slots")?;
    let [resource, empty, repeated_resource] = resources.as_slice() else {
        return Err("three ordered resource slots".into());
    };
    assert_eq!(
        resource
            .pointer("/value/targetRecordIndex/value")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        empty.pointer("/state").and_then(Value::as_str),
        Some("null")
    );
    assert_eq!(resource, repeated_resource);

    Ok(())
}

fn assert_parent_links(parents: &Value) -> TestResult {
    for physical in [1, 5] {
        let parent = row(parents, physical)?;
        assert_eq!(
            parent
                .pointer("/parent/value/targetRecordIndex/value")
                .and_then(Value::as_u64),
            Some(0)
        );
        assert_eq!(
            parent
                .pointer("/parent/containsEncryptedSource")
                .and_then(Value::as_bool),
            Some(true)
        );
    }
    Ok(())
}

#[test]
fn resource_groups_require_present_members_in_reduced_targets() -> TestResult {
    with_queries(|queries, scratch| {
        let mut texts = SelectedTexts::default();
        let resources = RecordTargets::default();
        assert!(matches!(
            resource_groups::collect(queries, scratch, None, &resources, &mut texts),
            Err(ReducedError::UnresolvedRecordLink {
                path: "declarations.resourceGroupMembers",
                index: 3,
            })
        ));
        Ok(())
    })
}

fn with_queries(
    verify: impl FnOnce(&ParsedCatalogQueries<'_>, &std::path::Path) -> TestResult,
) -> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    let name = GlobalIniBuilder::new().text("shared", "Metal").build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", fixture::graph().build()).encrypted())
        .entry(TestEntry::stored(Language::English.global_ini_path(), name))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let session = CatalogQuerySession::open(
        &archive,
        CatalogLanguageSelection::Selected(&[Language::English]),
        None,
    )?;
    verify(&session.parsed(None)?, scratch.path())
}

fn row(table: &Value, record_index: u64) -> TestResult<&Value> {
    table
        .get("rows")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|row| row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(record_index))
        .ok_or_else(|| format!("missing physical ResourceGroup Record {record_index}").into())
}
