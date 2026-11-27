// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::{DataForge, RecordTree, Value as OriginalValue, ValueError};
use p4kx_testkit::{DataForgeBuilder, TestValue};
use serde_json::{Value, json};

use crate::catalog_tags_fixture::{self as fixture, TestResult, at};
use crate::commodity_v3_source_inventory::{Use, body, header, reference_closure, uses};
use crate::health_origin_assertions;

const BAD_NAME: u32 = u32::MAX - 1;

#[derive(Clone, Copy)]
enum RootName {
    Present,
    Unreadable,
}

#[test]
fn tag_topology_preserves_selected_qualifiers_and_the_physical_parent() -> TestResult {
    for name in [RootName::Present, RootName::Unreadable] {
        let catalog = catalog(name)?;
        let path = fixture::row(&catalog, "projections/tagPaths", 1)?;
        assert_topology_values(path)?;
        assert_selected_uses(&catalog, path)?;
        assert_path(&catalog, path, name)?;
        reference_closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn readable_tag_names_do_not_contribute_to_topology() -> TestResult {
    let catalog = catalog(RootName::Present)?;
    let path = fixture::row(&catalog, "projections/tagPaths", 1)?;
    assert_topology_values(path)?;
    assert_selected_uses(&catalog, path)?;
    assert_path(&catalog, path, RootName::Present)?;
    assert_exact_topology(&catalog, path)
}

#[test]
fn an_unreadable_name_does_not_make_selected_topology_incomplete() -> TestResult {
    let catalog = catalog(RootName::Unreadable)?;
    let path = fixture::row(&catalog, "projections/tagPaths", 1)?;
    assert_topology_values(path)?;
    assert_selected_uses(&catalog, path)?;
    assert_path(&catalog, path, RootName::Unreadable)?;
    assert_exact_topology(&catalog, path)
}

fn catalog(name: RootName) -> TestResult<Value> {
    let database = database(name);
    assert_original(&database.build(), name)?;
    let (_, catalog) = fixture::diagnostic(&database)?;
    Ok(catalog)
}

fn database(name: RootName) -> DataForgeBuilder {
    let root = fixture::tag(
        fixture::ROOT,
        "Root",
        TestValue::Array(vec![TestValue::Null, TestValue::Reference(fixture::LEAF)]),
    );
    let root = match name {
        RootName::Present => root,
        RootName::Unreadable => {
            root.value("tagName", TestValue::Raw(BAD_NAME.to_le_bytes().to_vec()))
        }
    };
    fixture::schema()
        .record(root)
        .record(fixture::tag(
            fixture::LEAF,
            "Leaf",
            fixture::references(&[]),
        ))
        .record(fixture::roots(TestValue::Array(vec![
            TestValue::Null,
            TestValue::Reference(fixture::ROOT),
            TestValue::Reference(fixture::LEAF),
        ])))
}

fn assert_original(bytes: &[u8], name: RootName) -> TestResult {
    let forge = DataForge::open(bytes)?;
    assert_eq!(
        forge.records().count(),
        3,
        "all physical source Records are present"
    );
    let root = forge.record(0).ok_or("original root Tag")?;
    let leaf = forge.record(1).ok_or("original child Tag")?;
    let database = forge.record(2).ok_or("original TagDatabase")?;
    assert_eq!(
        root.guid().to_string(),
        fixture::guid(fixture::ROOT),
        "the fixture root has the original Tag GUID"
    );
    assert_eq!(
        leaf.guid().to_string(),
        fixture::guid(fixture::LEAF),
        "the fixture child has the original Tag GUID"
    );
    assert_eq!(
        database.guid().to_string(),
        fixture::guid(fixture::DATABASE),
        "the fixture database has the original TagDatabase GUID"
    );

    let root = root.tree()?;
    assert_root_name(&root, name);
    let leaf = leaf.tree()?;
    assert_eq!(
        leaf.error_count(),
        0,
        "the original child has no source errors"
    );
    assert!(
        matches!(leaf.field("tagName"), Some(OriginalValue::String("Leaf"))),
        "the original child name is readable"
    );
    let Some(OriginalValue::Array(children)) = root.field("children") else {
        return Err("original children".into());
    };
    let [OriginalValue::Null, child] = children.as_ref() else {
        return Err("actual null followed by the selected child".into());
    };
    edge(child, fixture::LEAF, 1)?;

    let database = database.tree()?;
    assert_eq!(
        database.error_count(),
        0,
        "the original database has no source errors"
    );
    let Some(OriginalValue::Array(roots)) = database.field("tags") else {
        return Err("original roots".into());
    };
    let [OriginalValue::Null, selected, repeated] = roots.as_ref() else {
        return Err("actual root and the later repeated occurrence".into());
    };
    edge(selected, fixture::ROOT, 0)?;
    edge(repeated, fixture::LEAF, 1)
}

fn assert_root_name(root: &RecordTree<'_>, name: RootName) {
    match name {
        RootName::Present => {
            assert_eq!(
                root.error_count(),
                0,
                "the readable root has no source errors"
            );
            assert!(
                matches!(root.field("tagName"), Some(OriginalValue::String("Root"))),
                "the original root name is readable"
            );
        }
        RootName::Unreadable => {
            assert_eq!(
                root.error_count(),
                1,
                "only the actual root name is unreadable"
            );
            assert!(
                matches!(root.field("tagName"),
                Some(OriginalValue::Error(ValueError::InvalidString { offset, .. })) if *offset == BAD_NAME),
                "the unreadable name uses the exact invalid string offset"
            );
        }
    }
}

fn edge(value: &OriginalValue<'_>, guid: u128, index: u32) -> TestResult {
    let OriginalValue::Reference(target) = value else {
        return Err("actual resolved edge".into());
    };
    assert_eq!(
        target.guid().to_string(),
        fixture::guid(guid),
        "the stored edge has its expected target GUID"
    );
    assert_eq!(
        target.record_index(),
        index,
        "the exact original target Record"
    );
    Ok(())
}

fn assert_topology_values(path: &Value) -> TestResult {
    assert_eq!(
        at(path, "/guid/value")?,
        &json!(fixture::guid(fixture::LEAF)),
        "the path row identifies the selected child Tag"
    );
    assert_eq!(
        at(path, "/recordIndex/value")?,
        &json!(1),
        "the path row identifies the physical child Record"
    );
    assert_eq!(
        at(path, "/depth/value")?,
        &json!(1),
        "the selected child is one edge below the root"
    );
    assert_eq!(
        at(path, "/listed/value")?,
        &json!(true),
        "the selected child belongs to the listed tree"
    );
    assert_eq!(
        at(path, "/parent/value/type")?,
        &json!("tag"),
        "the selected child has a Tag parent"
    );
    assert_eq!(
        at(path, "/parent/value/guid/value")?,
        &json!(fixture::guid(fixture::ROOT)),
        "the parent has the original root Tag GUID"
    );
    assert_eq!(
        at(path, "/parent/value/recordIndex/value")?,
        &json!(0),
        "the parent identifies the physical root Record"
    );
    Ok(())
}

fn qualifiers() -> BTreeSet<Use> {
    let mut expected = BTreeSet::from([
        (0, body("children[1]"), "selector".to_owned()),
        (2, body("tags[1]"), "selector".to_owned()),
    ]);
    for record in [0, 1, 2] {
        for field in ["guid", "sourceType", "recordIndex"] {
            expected.insert((record, header(field), "selector".to_owned()));
        }
    }
    expected
}

fn topology_uses(field: &str) -> BTreeSet<Use> {
    let mut expected = qualifiers();
    if field == "parent" {
        expected.insert((0, header("guid"), "payload".to_owned()));
    }
    expected
}

fn assert_selected_uses(catalog: &Value, path: &Value) -> TestResult {
    for field in ["depth", "listed", "parent"] {
        let actual = uses(catalog, at(path, &format!("/{field}"))?)?;
        assert!(
            topology_uses(field).is_subset(&actual),
            "{field} retains all actual qualifiers"
        );
        assert!(
            !actual.contains(&(2, body("tags[2]"), "selector".to_owned())),
            "the later original occurrence did not select {field}"
        );
        assert!(
            !actual.contains(&(0, body("children[0]"), "selector".to_owned())),
            "the null slot did not select {field}"
        );
    }
    for (field, role) in [("guid", "payload"), ("recordIndex", "selector")] {
        let cell = at(path, &format!("/parent/value/{field}"))?;
        assert_eq!(
            uses(catalog, cell)?,
            BTreeSet::from([(0, header(field), role.to_owned())]),
            "the parent header comes from the selected physical Record"
        );
    }
    Ok(())
}

fn assert_exact_topology(catalog: &Value, path: &Value) -> TestResult {
    for field in ["depth", "listed", "parent"] {
        let cell = at(path, &format!("/{field}"))?;
        assert_eq!(
            at(cell, "/sourceSetComplete")?,
            &json!(true),
            "{field} has complete selected topology even if name text is unreadable"
        );
        assert_eq!(
            at(cell, "/containsEncryptedSource")?,
            &json!(false),
            "{field} comes from the plain original qualifying inputs"
        );
        assert_eq!(
            uses(catalog, cell)?,
            topology_uses(field),
            "{field} consumes the exact selected topology and parent evidence"
        );
    }
    Ok(())
}

fn assert_path(catalog: &Value, path: &Value, name: RootName) -> TestResult {
    let cell = at(path, "/path")?;
    let mut expected = qualifiers();
    expected.insert((1, body("tagName"), "conversion".to_owned()));
    match name {
        RootName::Present => {
            expected.insert((0, body("tagName"), "conversion".to_owned()));
            assert_eq!(
                at(cell, "/value")?,
                &json!("Root.Leaf"),
                "the path uses both actual readable names"
            );
            assert_eq!(
                at(cell, "/sourceSetComplete")?,
                &json!(true),
                "both name inputs were read successfully"
            );
            assert_eq!(
                at(cell, "/containsEncryptedSource")?,
                &json!(false),
                "the complete path uses only proven unencrypted sources"
            );
        }
        RootName::Unreadable => {
            assert_eq!(
                at(cell, "/value")?,
                &json!(".Leaf"),
                "the unreadable root contributes an empty path segment"
            );
            assert_eq!(
                at(cell, "/sourceSetComplete")?,
                &json!(false),
                "the unreadable root name leaves path origin incomplete"
            );
            assert_eq!(
                at(cell, "/containsEncryptedSource")?,
                &Value::Null,
                "an incomplete path cannot prove entirely unencrypted origin"
            );
            assert_attempted_name(catalog)?;
        }
    }
    assert_eq!(
        uses(catalog, cell)?,
        expected,
        "the actual path text depends on its qualifying edges and readable name conversions"
    );
    Ok(())
}

fn assert_attempted_name(catalog: &Value) -> TestResult {
    let inventory = at(catalog, "/provenance/uses")?
        .as_array()
        .ok_or("original use inventory")?;
    let mut matched = inventory.iter().filter(|used| {
        used.pointer("/location/type") == Some(&json!("dataForge"))
            && used.pointer("/location/recordIndex") == Some(&json!(0))
            && used.pointer("/location/field") == Some(&json!("tagName"))
            && used.get("role") == Some(&json!("attempted"))
    });
    let used = matched
        .next()
        .ok_or("actual attempted read of the original root name")?;
    assert!(
        matched.next().is_none(),
        "the attempted physical use is distinct"
    );
    assert_eq!(
        at(used, "/scope")?,
        &json!("payload"),
        "the failed name read used the actual payload scope"
    );
    let source = health_origin_assertions::source(catalog, used)?;
    assert_eq!(
        at(source, "/kind")?,
        &json!("archiveEntry"),
        "the failed name read belongs to an archive Entry"
    );
    assert_eq!(
        at(source, "/path/value")?,
        &json!("Data\\Game2.dcb"),
        "the attempted name read identifies its original DataForge Entry"
    );
    Ok(())
}
