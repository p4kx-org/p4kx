// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::CatalogPublicationMode;
use serde_json::{Value, json};

use crate::catalog_tags_fixture::{self as fixture, TestResult, at};
use crate::commodity_v3_fixture::DcbOrigin;
use crate::commodity_v3_source_inventory::{body, header};
use crate::commodity_v3_source_inventory::{reference_closure, uses};

#[test]
fn direct_headers_names_arrays_and_reference_fields_retain_separate_physical_uses() -> TestResult {
    let (_, catalog) = fixture::diagnostic(&fixture::graph())?;
    let row = fixture::row(&catalog, "definitions/tags", 0)?;
    for (field, source, role) in [
        ("guid", header("guid"), "payload"),
        ("recordName", header("recordName"), "payload"),
        ("recordPath", header("fileName"), "conversion"),
        ("recordIndex", header("recordIndex"), "selector"),
        ("sourceType", header("sourceType"), "selector"),
        ("tagName", body("tagName"), "payload"),
        ("legacyGuid", body("legacyGUID"), "payload"),
        ("children", body("children"), "payload"),
    ] {
        assert_eq!(
            uses(&catalog, at(row, &format!("/{field}"))?)?,
            BTreeSet::from([(0, source, role.to_owned())])
        );
    }
    let child = at(row, "/children/value/3")?;
    assert_eq!(
        uses(&catalog, child)?,
        BTreeSet::from([(0, body("children[3]"), "payload".to_owned())])
    );
    assert_eq!(
        uses(&catalog, at(child, "/value/guid")?)?,
        BTreeSet::from([(0, body("children[3]"), "payload".to_owned())])
    );
    let target_index = at(child, "/value/targetRecordIndex/value")?
        .as_u64()
        .ok_or("target index")?;
    for (field, source, role) in [
        ("targetRecordIndex", "recordIndex", "selector"),
        ("targetType", "sourceType", "selector"),
        ("recordPath", "fileName", "conversion"),
    ] {
        assert_eq!(
            uses(&catalog, at(child, &format!("/value/{field}"))?)?,
            BTreeSet::from([(target_index, header(source), role.to_owned())])
        );
    }
    assert_eq!(
        at(child, "/containsEncryptedSource")?,
        &json!(false),
        "unrelated encrypted locales do not taint a plain authored edge"
    );
    reference_closure(&catalog)?;
    Ok(())
}

#[test]
fn convenience_path_uses_only_the_actual_chosen_occurrences_and_ancestor_names() -> TestResult {
    let (_, catalog) = fixture::diagnostic(&fixture::graph())?;
    let row = fixture::row(&catalog, "projections/tagPaths", 1)?;
    let actual = uses(&catalog, at(row, "/path")?)?;
    let mut topology = BTreeSet::from([
        (4, body("tags[2]"), "selector".to_owned()),
        (0, body("children[3]"), "selector".to_owned()),
    ]);
    for record in [0, 1, 4] {
        for field in ["guid", "sourceType", "recordIndex"] {
            topology.insert((record, header(field), "selector".to_owned()));
        }
    }
    let mut expected = topology.clone();
    expected.extend([
        (0, body("tagName"), "conversion".to_owned()),
        (1, body("tagName"), "conversion".to_owned()),
    ]);
    assert_eq!(
        actual, expected,
        "no compressed ordinal or counterfactual later edge contributes"
    );
    let root = fixture::row(&catalog, "projections/tagPaths", 0)?;
    assert!(
        !uses(&catalog, at(root, "/path")?)?
            .iter()
            .any(|(index, _, _)| *index == 1),
        "descendant names do not contribute to the root path"
    );
    for field in ["/depth", "/listed"] {
        assert_eq!(
            uses(&catalog, at(row, field)?)?,
            topology,
            "{field} depends on the actual qualifying headers and selected edges"
        );
    }
    topology.insert((0, header("guid"), "payload".to_owned()));
    assert_eq!(
        uses(&catalog, at(row, "/parent")?)?,
        topology,
        "the parent adds its actual GUID payload to the selected topology"
    );
    let unlisted = uses(
        &catalog,
        at(fixture::row(&catalog, "projections/tagPaths", 3)?, "/path")?,
    )?;
    assert!(
        unlisted.iter().all(|(index, _, _)| *index == 3),
        "unlisted fallback creates no invented root-array use"
    );
    Ok(())
}

#[test]
fn entity_root_array_and_repeated_elements_use_their_actual_owning_record() -> TestResult {
    let (_, catalog) = fixture::diagnostic(&fixture::entities())?;
    let row = fixture::rows(&catalog, "declarations/entityTags")?
        .iter()
        .find(|row| row["guid"]["value"] == fixture::guid(fixture::ITEM))
        .ok_or("item")?;
    let index = at(row, "/recordIndex/value")?
        .as_u64()
        .ok_or("owner index")?;
    assert_eq!(
        uses(&catalog, at(row, "/tags")?)?,
        BTreeSet::from([(index, body("tags"), "payload".to_owned())])
    );
    for ordinal in 0..3 {
        let element = at(row, &format!("/tags/value/{ordinal}"))?;
        assert_eq!(
            uses(&catalog, element)?,
            BTreeSet::from([(
                index,
                body(&format!("tags[{ordinal}]")),
                "payload".to_owned()
            )])
        );
    }
    reference_closure(&catalog)?;
    Ok(())
}

#[test]
fn encrypted_and_unknown_original_dcb_origins_reach_nested_facts_and_derived_paths() -> TestResult {
    for (origin, expected) in [
        (DcbOrigin::Encrypted, json!(true)),
        (DcbOrigin::Unknown, Value::Null),
    ] {
        let (_, catalog) = fixture::build(
            &fixture::entities(),
            origin,
            CatalogPublicationMode::DiagnosticPartial,
            None,
        )?;
        for table in [
            "definitions/tags",
            "declarations/tagRoots",
            "declarations/entityTags",
            "projections/tagPaths",
        ] {
            for row in fixture::rows(&catalog, table)? {
                assert_carriers(row, &expected)?;
            }
        }
        reference_closure(&catalog)?;
    }
    Ok(())
}

fn assert_carriers(value: &Value, expected: &Value) -> TestResult {
    if value.get("state").is_some() && value.get("sourceUsesRefs").is_some() {
        assert_eq!(
            at(value, "/containsEncryptedSource")?,
            expected,
            "each nested sourced Tag datum retains the original DCB encryption marker"
        );
    }
    match value {
        Value::Object(object) => {
            for child in object.values() {
                assert_carriers(child, expected)?;
            }
        }
        Value::Array(array) => {
            for child in array {
                assert_carriers(child, expected)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[test]
fn equal_database_guids_keep_both_physical_root_declarations_and_the_selected_owner() -> TestResult
{
    let database = fixture::graph().record(fixture::roots(fixture::references(&[fixture::LOOSE])));
    let (_, catalog) = fixture::diagnostic(&database)?;
    let roots = fixture::rows(&catalog, "declarations/tagRoots")?;
    assert_eq!(
        roots.len(),
        2,
        "nominal root-database identity does not merge physical declarations"
    );
    assert_eq!(
        at(
            fixture::row(&catalog, "declarations/tagRoots", 4)?,
            "/guid/value"
        )?,
        at(
            fixture::row(&catalog, "declarations/tagRoots", 7)?,
            "/guid/value"
        )?
    );
    let path = fixture::row(&catalog, "projections/tagPaths", 3)?;
    assert_eq!(
        at(path, "/listed/value")?,
        &json!(true),
        "the second actual root listing is used"
    );
    let inputs = uses(&catalog, at(path, "/path")?)?;
    assert!(
        inputs.contains(&(7, body("tags[0]"), "selector".to_owned())),
        "original physical database owns the selected incoming slot"
    );
    assert!(
        !inputs.iter().any(|(index, _, _)| *index == 4),
        "equal GUIDs never restore another source owner's authority"
    );
    Ok(())
}

#[path = "catalog_tag_name_origin.rs"]
mod name_origin;

#[path = "catalog_tag_topology_origin.rs"]
mod topology_origin;

#[path = "catalog_tag_dependent_receipts.rs"]
mod dependent_receipts;
