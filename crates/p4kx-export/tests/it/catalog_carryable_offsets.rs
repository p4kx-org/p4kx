// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ptr;

use p4kx_export::build_catalog_from;
use p4kx_export::catalog_queries::{CatalogQuerySession, CatalogRecordSelection, ProvenanceMode};
use p4kx_extraction::{Archive, DataForgeEntry, Field, RecordTree, StructValue, Value as Original};
use serde_json::{Value, json};

use super::{DcbOrigin, Grips, TestResult, common, database, full_proof, proof};

#[test]
fn hand_offsets_match_exact_original_axes_in_both_profiles() -> TestResult {
    let (_directory, archive) = common::open(&database(Grips::Present), DcbOrigin::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let owner = forge
        .record(0)
        .ok_or("the physical fixture owner")?
        .tree()?;
    let grips = original_grips(&owner)?;

    let normal = common::options();
    let marker_catalog: Value =
        serde_json::from_slice(build_catalog_from(&archive, &entry, &normal)?.json())?;
    let full_catalog: Value = serde_json::from_slice(
        build_catalog_from(&archive, &entry, normal.with_full_provenance())?.json(),
    )?;
    let table = "/configurations/interactions/rows/0";
    let full_row = proof::at(&full_catalog, table)?;
    let marker_row = proof::at(proof::at(&marker_catalog, "/catalog")?, table)?;
    check_grips(&full_catalog, owner.fields(), grips, full_row, marker_row)?;
    check_marker_sdk(&archive, &entry, marker_row)
}

fn original_grips<'tree, 'source>(
    owner: &'tree RecordTree<'source>,
) -> TestResult<&'tree [Original<'source>]> {
    let Some(Original::Array(components)) = owner.field("Components") else {
        return Err("original Components array".into());
    };
    let Some(Original::Struct(component)) = components.get(1) else {
        return Err("original carryable component".into());
    };
    let Some(Original::Struct(container)) = component.field("gripData") else {
        return Err("original grip container".into());
    };
    let Some(Original::Array(grips)) = container.field("grips") else {
        return Err("original grips array".into());
    };
    Ok(grips)
}

fn check_grips(
    catalog: &Value,
    fields: &[Field<'_>],
    grips: &[Original<'_>],
    full_row: &Value,
    marker_row: &Value,
) -> TestResult {
    let original_grips = proof::rows(full_row, "/gripData/value/grips/value")?;
    assert_eq!(
        original_grips.len(),
        grips.len(),
        "every original grip occupies its ordered slot"
    );
    for (index, grip) in grips.iter().enumerate() {
        let Original::Struct(grip) = grip else {
            return Err("original grip element".into());
        };
        for hand in ["handGrip", "offHandGrip"] {
            let path = format!("/gripData/value/grips/value/{index}/value/{hand}");
            check_hand(
                catalog,
                fields,
                grip.field(hand),
                proof::at(full_row, &path)?,
                proof::at(marker_row, &path)?,
            )?;
        }
    }
    Ok(())
}

fn check_hand(
    catalog: &Value,
    fields: &[Field<'_>],
    original: Option<&Original<'_>>,
    full_hand: &Value,
    marker_hand: &Value,
) -> TestResult {
    let hand = match original {
        Some(Original::Null) => {
            check_null(full_hand, marker_hand);
            return Ok(());
        }
        Some(Original::Struct(hand)) => hand,
        _ => return Err("original hand declaration".into()),
    };
    let Some(Original::Struct(offset)) = hand.field("offset") else {
        return Err("original local offset".into());
    };
    let full_offset = proof::at(full_hand, "/value/offset/value")?;
    let marker_offset = proof::at(marker_hand, "/value/offset/value")?;
    for (source_field, field) in [("Position", "position"), ("Rotation", "rotation")] {
        let Some(Original::Struct(axes)) = offset.field(source_field) else {
            return Err("original offset vector".into());
        };
        let path = format!("/{field}/value");
        check_axes(
            catalog,
            fields,
            axes,
            proof::at(full_offset, &path)?,
            proof::at(marker_offset, &path)?,
        )?;
    }
    Ok(())
}

fn check_marker_sdk(archive: &Archive, entry: &DataForgeEntry, marker_row: &Value) -> TestResult {
    let session = CatalogQuerySession::from_entry(archive, entry, full_proof::NO_LANGUAGES, None)?;
    session.with_interactions(
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| -> TestResult {
            let [row] = snapshot.rows() else {
                return Err("one scoped carryable row".into());
            };
            assert_eq!(
                snapshot.source_inventory().provenance_mode(),
                ProvenanceMode::MarkersOnly,
                "the default SDK uses Marker provenance"
            );
            assert_eq!(
                &serde_json::to_value(row)?,
                marker_row,
                "the Marker SDK and Catalog preserve the same local offsets"
            );
            Ok(())
        },
    )?
}

fn check_null(full: &Value, markers: &Value) {
    assert_eq!(full["state"], "null", "the Full optional grip remains null");
    assert_eq!(
        markers["state"], "null",
        "the Marker optional grip remains null"
    );
    assert!(markers["value"].is_null(), "a null grip has no offset");
}

fn check_axes(
    catalog: &Value,
    fields: &[Field<'_>],
    axes: &StructValue<'_>,
    full: &Value,
    marker: &Value,
) -> TestResult {
    for axis in ["x", "y", "z"] {
        let Some(original @ Original::Single(number)) = axes.field(axis) else {
            return Err("original Single axis".into());
        };
        let value = json!({"rawBits": format!("{:08x}", number.to_bits()), "finite": number});
        let cell = proof::present(full, axis, &value, Some(true))?;
        let coordinate = value_ordinal(fields, original).ok_or("physical original axis ordinal")?;
        assert!(
            proof::uses(catalog, cell)?.iter().any(|used| {
                used["scope"] == "payload"
                    && used["role"] == "payload"
                    && used.pointer("/location/namespace") == Some(&json!("dataforgeValue"))
                    && used.pointer("/location/indices")
                        == Some(&json!(["0", coordinate.to_string()]))
                    && used.pointer("/location/field") == Some(&json!("$value"))
            }),
            "the Full {axis} axis points to its independently traversed original occurrence"
        );
        assert_eq!(
            marker[axis],
            json!({"state": "present", "value": value, "containsEncryptedSource": true}),
            "the Marker {axis} axis preserves bits and encryption"
        );
    }
    Ok(())
}

fn value_ordinal<'source>(fields: &[Field<'source>], sought: &Original<'source>) -> Option<usize> {
    let mut pending: Vec<_> = fields.iter().rev().map(Field::value).collect();
    let mut ordinal = 0;
    while let Some(value) = pending.pop() {
        if ptr::eq(value, sought) {
            return Some(ordinal);
        }
        match value {
            Original::Struct(node) => pending.extend(node.fields().iter().rev().map(Field::value)),
            Original::Array(values) => pending.extend(values.iter().rev()),
            _ => {}
        }
        ordinal += 1;
    }
    None
}
