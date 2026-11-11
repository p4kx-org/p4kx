// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, Filter};
use serde_json::{Value, json};

use crate::catalog_tags_fixture::{self as fixture, TestResult, at};
use crate::commodity_v3_fixture::DcbOrigin;

#[test]
fn every_physical_tag_and_every_authored_edge_survives_the_convenience_tree() -> TestResult {
    let (built, catalog) = fixture::diagnostic(&fixture::graph())?;
    let rows = fixture::rows(&catalog, "definitions/tags")?;
    assert_eq!(
        rows.len(),
        5,
        "duplicate GUIDs retain their physical definitions"
    );
    assert!(
        rows.iter().all(|row| row.get("physical").is_none()),
        "private source keys never become wire fields"
    );
    assert_eq!(at(&catalog, "/schemaVersion")?, &json!(3));
    assert!(catalog.get("tags").is_none(), "one canonical tag dataset");
    assert_eq!(
        at(
            fixture::row(&catalog, "definitions/tags", 0)?,
            "/legacyGuid/value"
        )?,
        &json!(u32::MAX)
    );
    assert_raw_edges(&catalog)?;
    assert_definition_order_and_names(&catalog, rows)?;
    let health = built
        .summary()
        .export_health()
        .kinds()
        .iter()
        .find(|kind| kind.kind.name() == "tags")
        .ok_or("tag health")?;
    assert_eq!(
        health.written_rows, 5,
        "paths and root declarations are not extra selected Tag rows"
    );
    assert_eq!(health.source_records.selected, 5);
    Ok(())
}

fn assert_raw_edges(catalog: &Value) -> TestResult {
    let child = at(
        fixture::row(catalog, "definitions/tags", 0)?,
        "/children/value",
    )?
    .as_array()
    .ok_or("children")?;
    assert_eq!(child.len(), 5, "all original child slots remain available");
    assert_eq!(
        at(child.first().ok_or("child ordinal")?, "/state")?,
        &json!("null"),
        "a null child keeps its original slot"
    );
    assert_eq!(
        at(child.get(2).ok_or("child ordinal")?, "/value/guid/value")?,
        &json!(fixture::guid(fixture::MISSING)),
        "a missing child retains its authored GUID"
    );
    assert_eq!(
        at(child.get(2).ok_or("child ordinal")?, "/value/status")?,
        &json!("dangling"),
        "a missing child keeps an explicit dangling status"
    );
    assert_eq!(
        at(child.get(1).ok_or("child ordinal")?, "/value/status")?,
        &json!("unexpectedTargetType"),
        "a child of another type keeps its source status"
    );
    assert_eq!(
        at(child.get(3).ok_or("child ordinal")?, "/value/guid/value")?,
        at(child.get(4).ok_or("child ordinal")?, "/value/guid/value")?,
        "repeated child references retain both positions"
    );
    let roots = at(
        fixture::row(catalog, "declarations/tagRoots", 4)?,
        "/tags/value",
    )?
    .as_array()
    .ok_or("roots")?;
    assert_eq!(roots.len(), 5, "all original root slots remain available");
    assert_eq!(
        at(roots.first().ok_or("root ordinal")?, "/state")?,
        &json!("null"),
        "a null root keeps its original slot"
    );
    assert_eq!(
        at(roots.get(2).ok_or("root ordinal")?, "/value/guid/value")?,
        at(roots.get(4).ok_or("root ordinal")?, "/value/guid/value")?,
        "repeated root references retain both positions"
    );
    Ok(())
}

fn assert_definition_order_and_names(catalog: &Value, rows: &[Value]) -> TestResult {
    let physical: Vec<_> = rows
        .iter()
        .map(|row| row.pointer("/recordIndex/value").and_then(Value::as_u64))
        .collect();
    assert_eq!(
        physical,
        [Some(0), Some(1), Some(5), Some(2), Some(3)],
        "GUID then physical index"
    );
    assert_eq!(
        at(
            fixture::row(catalog, "definitions/tags", 1)?,
            "/tagName/value"
        )?,
        at(
            fixture::row(catalog, "definitions/tags", 2)?,
            "/tagName/value"
        )?,
        "same-name Tags remain separate physical definitions"
    );
    assert_eq!(
        at(
            fixture::row(catalog, "definitions/tags", 5)?,
            "/tagName/value"
        )?,
        &json!("Shadow"),
        "a duplicate GUID retains its own authored name"
    );
    Ok(())
}

#[test]
fn first_selected_paths_expose_their_selected_parent_and_unlisted_state() -> TestResult {
    let (_, catalog) = fixture::diagnostic(&fixture::graph())?;
    let paths = fixture::rows(&catalog, "projections/tagPaths")?;
    assert_eq!(
        paths.len(),
        4,
        "duplicate physical GUID is not a second convenience node"
    );
    let values: Vec<_> = paths
        .iter()
        .map(|row| row.pointer("/path/value").and_then(Value::as_str))
        .collect();
    assert_eq!(
        values,
        [Some("Root"), Some("Root.Same"), Some("Same"), Some("Loose")]
    );
    let leaf = fixture::row(&catalog, "projections/tagPaths", 1)?;
    assert_eq!(at(leaf, "/depth/value")?, &json!(1));
    assert_eq!(at(leaf, "/parent/value/type")?, &json!("tag"));
    assert_eq!(at(leaf, "/parent/value/recordIndex/value")?, &json!(0));
    assert_eq!(
        at(
            fixture::row(&catalog, "projections/tagPaths", 3)?,
            "/listed/value"
        )?,
        &json!(false)
    );
    let diagnostics = at(&catalog, "/exportHealth/diagnostics")?
        .as_array()
        .ok_or("diagnostics")?;
    assert!(
        diagnostics
            .iter()
            .any(|row| row["warningCode"] == "tag_duplicate_guid"),
        "physical duplicate has its own reason"
    );
    assert!(
        diagnostics
            .iter()
            .any(|row| row["warningCode"] == "tag_relation_repeated"),
        "repeated and cycle-cut relations remain reported"
    );
    assert!(
        diagnostics
            .iter()
            .any(|row| row["warningCode"] == "tag_relation_dangling"),
        "missing source target has an explicit reason"
    );
    Ok(())
}

#[test]
fn selected_entities_keep_root_guids_separate_from_attach_words_and_each_other() -> TestResult {
    let (_, catalog) = fixture::diagnostic(&fixture::entities())?;
    let entities = fixture::rows(&catalog, "declarations/entityTags")?;
    assert_eq!(
        entities.len(),
        3,
        "item, ship and canonical commodity owners"
    );
    let item = entities
        .iter()
        .find(|row| row["guid"]["value"] == fixture::guid(fixture::ITEM))
        .ok_or("item owner")?;
    let tags = at(item, "/tags/value")?.as_array().ok_or("entity tags")?;
    assert_eq!(tags.len(), 3);
    assert_eq!(
        at(&tags[0], "/value/guid/value")?,
        at(&tags[1], "/value/guid/value")?
    );
    assert_eq!(
        at(&tags[2], "/value/guid/value")?,
        &json!(fixture::guid(fixture::OTHER))
    );
    let item = at(&catalog, "/items")?
        .as_array()
        .and_then(|items| items.first())
        .ok_or("item")?;
    assert_eq!(at(item, "/tags")?, &json!(["flightReady", "$Small"]));
    let commodity = fixture::rows(&catalog, "definitions/commodities")?
        .first()
        .ok_or("commodity")?;
    assert!(
        commodity.get("tags").is_none(),
        "no duplicate root-tag payload in another flat view"
    );
    Ok(())
}

#[test]
fn entity_filter_does_not_filter_taxonomy_or_turn_its_references_into_missing_targets() -> TestResult
{
    let filter = Filter::new("included");
    let (built, catalog) = fixture::build(
        &fixture::entities(),
        DcbOrigin::Plain,
        CatalogPublicationMode::DiagnosticPartial,
        Some(&filter),
    )?;
    assert_eq!(fixture::rows(&catalog, "definitions/tags")?.len(), 3);
    assert_eq!(fixture::rows(&catalog, "declarations/entityTags")?.len(), 1);
    assert_eq!(
        at(&catalog, "/completeness/tags")?,
        &json!({"status":"complete"})
    );
    let kind = built
        .summary()
        .kinds()
        .iter()
        .find(|kind| kind.kind.name() == "tags")
        .ok_or("tag summary")?;
    assert!(
        kind.complete,
        "global taxonomy selection is complete under an Entity Filter"
    );
    assert_eq!(
        (kind.count, kind.skipped),
        (3, 0),
        "dev path does not discard taxonomy"
    );
    let unresolved = at(&catalog, "/exportHealth/unresolvedReferences")?
        .as_array()
        .ok_or("reference health")?;
    assert!(
        !unresolved
            .iter()
            .any(|reference| reference["targetType"] == "Tag"),
        "all tag targets were emitted globally"
    );
    Ok(())
}

#[test]
fn valid_tags_use_normal_publication_and_a_v3_consumer_may_ignore_the_additional_tables()
-> TestResult {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Consumer {
        schema_version: u32,
    }
    let (_, catalog) = fixture::build(
        &fixture::clean(),
        DcbOrigin::Plain,
        CatalogPublicationMode::Normal,
        None,
    )?;
    let consumer: Consumer = serde_json::from_value(catalog)?;
    assert_eq!(consumer.schema_version, 3);
    Ok(())
}
