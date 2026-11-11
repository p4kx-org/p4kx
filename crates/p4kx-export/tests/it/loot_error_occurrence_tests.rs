// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::time::UNIX_EPOCH;

use p4kx_export::{
    BuiltCatalog, CatalogError, CatalogOptions, CatalogPublicationMode, ExportHealthStatus,
};
use p4kx_extraction::{SourceInventory, SourceUseRole};
use serde_json::{Value, json};

use crate::loot_error_occurrence_fixture::{self as fixture, Case, Checked, Fixture, Storage};
use crate::loot_error_occurrence_source;
use crate::loot_v3_graph::{self as graph, Graph};
use crate::loot_v3_origin::{self as origin, ExpectedOrigin};

#[test]
fn every_repeated_error_fact_survives_beyond_aggregated_health_examples() -> Checked {
    for storage in [Storage::Plain, Storage::Encrypted] {
        let input = fixture::archive(Case::RepeatedBool, storage)?;
        loot_error_occurrence_source::check(&input, Case::RepeatedBool, storage)?;
        let refusal = refuse_normal(&input)?;
        let snapshot = refusal
            .source_inventory()
            .ok_or("paired immutable refusal snapshot")?;
        for ordinal in 1..=12 {
            let expected = origin::value_use(ordinal, "$error.byte", SourceUseRole::Payload);
            assert!(
                snapshot.uses().iter().any(|observed| {
                    observed.role() == expected.role && observed.location() == &expected.location
                }),
                "refusal snapshot retains every actually captured error fact"
            );
        }
        check_repeated_output(&diagnostic(&input)?, storage)?;
    }
    Ok(())
}

fn check_repeated_output(built: &BuiltCatalog, storage: Storage) -> Checked {
    let catalog: Value = serde_json::from_slice(built.json())?;
    let owner = Graph::new(&catalog, fixture::GUID_TEXT)?;
    check_count(&catalog, 13);
    let elements = graph::elements(owner.root("errors")?)?;
    assert_eq!(
        elements.len(),
        12,
        "all twelve identical original error slots survive"
    );
    let mut indexes = HashSet::new();
    for (slot, reference) in elements.iter().enumerate() {
        let index = reference
            .get("valueIndex")
            .and_then(Value::as_str)
            .ok_or("original occurrence reference")?;
        assert!(indexes.insert(index), "equal error facts are not interned");
        let node = owner.follow(reference)?;
        check_unreadable(node, "invalidBool");
        let byte = node
            .pointer("/originalError/byte")
            .ok_or("complete per-occurrence malformed byte")?;
        assert_eq!(
            graph::present(byte)?,
            &json!(129),
            "the exact original byte survives each occurrence"
        );
        require_origin(
            byte,
            built.summary().source_inventory(),
            storage,
            u64::try_from(slot)? + 1,
            "$error.byte",
            SourceUseRole::Payload,
        )?;
    }
    Ok(())
}

#[test]
fn class_array_ownership_error_uses_its_original_declaration_and_mapping() -> Checked {
    let input = fixture::archive(Case::ClassAlias, Storage::Plain)?;
    loot_error_occurrence_source::check(&input, Case::ClassAlias, Storage::Plain)?;
    refuse_normal(&input)?;
    let built = diagnostic(&input)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let owner = Graph::new(&catalog, fixture::GUID_TEXT)?;
    check_count(&catalog, 5);
    let elements = graph::elements(owner.root("right")?)?;
    assert_eq!(
        elements.len(),
        1,
        "duplicate original mapping retains its slot"
    );
    let node = owner.follow(elements.first().ok_or("actual original duplicate slot")?)?;
    check_unreadable(node, "ownedTwice");
    check_class_facts(node, built.summary().source_inventory())
}

fn check_class_facts(node: &Value, inventory: &SourceInventory) -> Checked {
    let structure = node
        .pointer("/originalError/structIndex")
        .ok_or("original declaration index")?;
    let instance = node
        .pointer("/originalError/instanceIndex")
        .ok_or("original mapping index")?;
    assert_eq!(
        graph::present(structure)?,
        &json!(1),
        "exact reported struct index"
    );
    assert_eq!(
        graph::present(instance)?,
        &json!(0),
        "zero original instance index survives"
    );
    require_origin(
        structure,
        inventory,
        Storage::Plain,
        3,
        "$target",
        SourceUseRole::Selector,
    )?;
    require_origin(
        instance,
        inventory,
        Storage::Plain,
        4,
        "$error.instanceIndex",
        SourceUseRole::Conversion,
    )
}

#[test]
fn dangling_guid_remains_exact_without_inventing_a_successful_reference() -> Checked {
    for storage in [Storage::Plain, Storage::Encrypted] {
        let input = fixture::archive(Case::Dangling, storage)?;
        loot_error_occurrence_source::check(&input, Case::Dangling, storage)?;
        let built = p4kx_export::build_catalog(&input.archive, &CatalogOptions::new(UNIX_EPOCH))?;
        assert_eq!(
            built.summary().export_health().status,
            ExportHealthStatus::Complete,
            "the existing informational dangling policy is unchanged"
        );
        let catalog: Value = serde_json::from_slice(built.json())?;
        let owner = Graph::new(&catalog, fixture::GUID_TEXT)?;
        check_count(&catalog, 1);
        let node = owner.root("broken")?;
        check_unreadable(node, "danglingReference");
        let guid = node
            .pointer("/originalError/guid")
            .ok_or("retained authored GUID")?;
        assert_eq!(
            graph::present(guid)?,
            &json!(fixture::DANGLING_TEXT),
            "no GUID loss or target admission is invented"
        );
        require_origin(
            guid,
            built.summary().source_inventory(),
            storage,
            0,
            "$error.guid",
            SourceUseRole::Payload,
        )?;
    }
    Ok(())
}

fn refuse_normal(input: &Fixture) -> Checked<CatalogError> {
    let Err(refusal) = p4kx_export::build_catalog(&input.archive, &CatalogOptions::new(UNIX_EPOCH))
    else {
        return Err("Normal must refuse the genuine source errors".into());
    };
    assert!(
        matches!(&refusal, CatalogError::IncompleteExport(_)),
        "source errors remain an explicit Normal refusal"
    );
    Ok(refusal)
}

fn diagnostic(input: &Fixture) -> Checked<BuiltCatalog> {
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = p4kx_export::build_catalog(&input.archive, &options)?;
    assert_eq!(
        built.summary().export_health().status,
        ExportHealthStatus::Incomplete,
        "retained facts do not make source errors readable"
    );
    Ok(built)
}

fn check_count(catalog: &Value, expected: usize) {
    assert_eq!(
        catalog
            .pointer("/definitions/lootPoolFilters/rows/0/values")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(expected),
        "every independently counted original occurrence, no ghost slots"
    );
}

fn check_unreadable(node: &Value, kind: &str) {
    assert_eq!(
        node.pointer("/value/state"),
        Some(&json!("unreadable")),
        "semantic source error remains unreadable"
    );
    assert_eq!(
        node.pointer("/value/value"),
        Some(&Value::Null),
        "no successful descriptor or value is invented"
    );
    assert_eq!(
        node.pointer("/originalError/type"),
        Some(&json!(kind)),
        "the exact original error variant survives"
    );
}

fn require_origin(
    cell: &Value,
    inventory: &SourceInventory,
    storage: Storage,
    ordinal: u64,
    part: &str,
    role: SourceUseRole,
) -> Checked {
    let uses = [origin::value_use(ordinal, part, role)];
    origin::verify(
        cell,
        inventory,
        &ExpectedOrigin {
            encrypted: Some(storage.encrypted()),
            complete: true,
            uses: &uses,
        },
    )
    .map_err(|failure| format!("exact original occurrence origin: {failure:?}").into())
}
