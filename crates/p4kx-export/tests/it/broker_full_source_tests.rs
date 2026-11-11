// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, Field, RecordTree, Value};
use serde_json::{Value as Json, json};

use crate::broker_flow_graph::Broker;
use crate::health_origin_fixture::{self, TestResult};
use crate::loot_v3_graph as raw;
use crate::spawns_fixture;

#[test]
fn one_physical_broker_arena_retains_every_original_root_and_value() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        spawns_fixture::write_archive(folder.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let input = DataForgeEntry::read(&archive)?;
    let forge = input.open()?;
    let identity = health_origin_fixture::guid(spawns_fixture::BROKER);
    let record = forge
        .records()
        .find(|record| record.guid().to_string() == identity)
        .ok_or("actual full original Broker record")?;
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "one original Broker record and its actual tree"
    );
    assert_eq!(
        record.type_name()?,
        "MissionBrokerEntry",
        "the true original family"
    );
    original_fields(&tree)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..health_origin_fixture::options()
    };
    let built = build_catalog_from(&archive, &input, &options)?;
    let document: Json = serde_json::from_slice(built.json())?;
    let graph = Broker::for_guid(&document, &identity)?;
    arena(&tree, &graph)?;
    let root = raw::array(graph.owner.source, "roots")?;
    let title = root
        .iter()
        .find(|field| field.pointer("/name/value") == Some(&json!("title")))
        .ok_or("actual retained original title declaration")?;
    let title = graph
        .owner
        .follow(title.get("value").ok_or("actual root value link")?)?;
    assert_eq!(
        crate::mission_template_graph::text(title)?,
        "@place_name",
        "the same original title"
    );
    assert!(
        document.get("missionBrokerEntries").is_none(),
        "there is no second raw Broker root table"
    );
    crate::health_origin_assertions::closure(&document)?;
    Ok(())
}

fn original_fields(tree: &RecordTree<'_>) -> TestResult {
    assert_eq!(
        tree.field("title"),
        Some(&Value::Locale("@place_name")),
        "the exact original title token"
    );
    assert_eq!(
        tree.field("missionDifficulty"),
        Some(&Value::Int32(3)),
        "the original scalar quantity"
    );
    assert_eq!(
        tree.field("personalCooldownTime"),
        Some(&Value::Single(60.0)),
        "the original Single"
    );
    assert_eq!(
        tree.field("initiallyActive"),
        Some(&Value::Bool(true)),
        "the original availability flag"
    );
    let Some(Value::Array(properties)) = tree.field("properties") else {
        return Err("actual original Broker properties array".into());
    };
    assert_eq!(
        properties.len(),
        4,
        "the four real original property declarations"
    );
    Ok(())
}

fn arena(tree: &RecordTree<'_>, graph: &Broker<'_>) -> TestResult {
    let roots = raw::array(graph.owner.source, "roots")?;
    assert_eq!(
        roots.len(),
        tree.fields().len(),
        "the one D owner retains every actual original root declaration"
    );
    for (position, (root, original)) in roots.iter().zip(tree.fields()).enumerate() {
        assert_eq!(
            raw::present(
                root.get("name")
                    .ok_or("actual original root declaration name")?
            )?,
            &json!(original.name()),
            "the original declaration name and order"
        );
        assert_eq!(
            root.get("sourcePosition"),
            Some(&json!({"parentOrdinal":null,"fieldIndex":position.to_string()})),
            "the genuine original Record-root slot, not an output index"
        );
        graph
            .owner
            .follow(root.get("value").ok_or("actual root value reference")?)?;
    }
    let mut pending: Vec<_> = tree.fields().iter().map(Field::value).collect();
    let mut count = 0;
    while let Some(value) = pending.pop() {
        count += 1;
        match value {
            Value::Array(elements) => pending.extend(elements.iter()),
            Value::Struct(owner) => pending.extend(owner.fields().iter().map(Field::value)),
            _ => {}
        }
    }
    assert_eq!(
        graph.owner.values.len(),
        count,
        "the source includes each actual original occurrence once, including null and decoder errors"
    );
    for (index, node) in graph.owner.values.iter().enumerate() {
        assert_eq!(
            crate::flow_owner_graph::ordinal(node)?,
            u64::try_from(index)?,
            "no source ordinal disappears or is replaced with equal content"
        );
    }
    Ok(())
}
