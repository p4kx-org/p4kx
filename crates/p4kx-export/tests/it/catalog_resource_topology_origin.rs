// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::{bindings, catalog_from, fixture, loadout, owner, proof};
use crate::commodity_v3_fixture::TestResult;

#[test]
fn derived_endpoints_bind_only_the_observed_name_scope() -> TestResult {
    for (source, endpoint, attribute) in [
        (fixture::GraphSource::Vehicle, 6, "@name"),
        (fixture::GraphSource::Loadout, 0, "@portName"),
        (fixture::GraphSource::Patch, 6, "@name"),
        (fixture::GraphSource::Elem, 6, "@value"),
    ] {
        let (_directory, archive) = fixture::graph_archive_for(source)?;
        let catalog = catalog_from(&archive)?;
        let (root, _) = loadout(&catalog, fixture::SHIP)?;
        let authored = owner(
            &catalog,
            "/declarations/resourceNetworkLinks/rows",
            fixture::SHIP,
        )?;
        let selector = proof::at(
            authored,
            &format!("/internalHardpointLinks/value/{endpoint}/value/port1"),
        )?;
        assert_eq!(
            selector["containsEncryptedSource"], false,
            "the original selector is in the clear DCB"
        );
        let links = bindings(&catalog, root)?;
        let binding = links[endpoint];
        let cell = proof::at(binding, "/port1")?;
        assert_eq!(
            cell["containsEncryptedSource"], true,
            "a consulted original name was encrypted"
        );
        assert_eq!(
            cell["sourceSetComplete"], true,
            "the direct original names are bound"
        );
        assert_eq!(
            proof::at(cell, "/value/state")?,
            "resolved",
            "the same original endpoint resolves"
        );
        assert_name_attribute(&catalog, cell, attribute)?;
        if matches!(source, fixture::GraphSource::Loadout) {
            assert_selected_loadout_attribute(&catalog, cell)?;
        }
        check_name_inventory(&catalog, links[4], attribute)?;
        let weak = links
            .iter()
            .find(|row| row["family"] == "resource")
            .ok_or("weak binding")?;
        assert_eq!(
            proof::at(weak, "/port1/containsEncryptedSource")?,
            &json!(false),
            "an unrelated XML name is outside the exact pointer lookup"
        );
        proof::closure(&catalog)?;
    }
    Ok(())
}

fn check_name_inventory(catalog: &Value, binding: &Value, attribute: &str) -> TestResult {
    for (path, state) in [("/port1", "ambiguous"), ("/port2", "missing")] {
        let cell = proof::at(binding, path)?;
        assert_eq!(
            proof::at(cell, "/value/state")?,
            state,
            "the complete direct name inventory supplies the result"
        );
        assert_eq!(
            cell["containsEncryptedSource"], true,
            "the consulted encrypted candidate remains evidence"
        );
        assert_eq!(
            cell["sourceSetComplete"], true,
            "the complete name scope is bound"
        );
        assert_name_attribute(catalog, cell, attribute)?;
    }
    Ok(())
}

fn assert_name_attribute(catalog: &Value, cell: &Value, attribute: &str) -> TestResult {
    let uses = proof::rows(catalog, "/provenance/uses")?;
    let references = proof::rows(cell, "/sourceUsesRefs")?;
    assert!(
        uses.iter().any(|use_row| {
            references.contains(&use_row["id"])
                && use_row["effectiveEncryption"] == "encrypted"
                && use_row["role"] == "selector"
                && use_row
                    .pointer("/location/namespace")
                    .and_then(Value::as_str)
                    == Some("xmlAttribute")
                && use_row.pointer("/location/field").and_then(Value::as_str) == Some(attribute)
        }),
        "the actual original XML attribute is an encrypted selector"
    );
    Ok(())
}

fn assert_selected_loadout_attribute(catalog: &Value, cell: &Value) -> TestResult {
    let uses = proof::rows(catalog, "/provenance/uses")?;
    let references = proof::rows(cell, "/sourceUsesRefs")?;
    let positions: Vec<_> = uses
        .iter()
        .filter(|used| {
            references.contains(&used["id"])
                && used.pointer("/location/namespace") == Some(&json!("xmlAttribute"))
                && used.pointer("/location/field") == Some(&json!("@portName"))
                && used.pointer("/location/indices/0") == Some(&json!("3"))
        })
        .filter_map(|used| used.pointer("/location/indices/1"))
        .collect();
    assert_eq!(
        positions,
        vec![&json!("0")],
        "the first physical local-name match"
    );
    Ok(())
}
