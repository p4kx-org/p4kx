// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::Transform;
use p4kx_extraction::{InputScope, SourceInput, SourceUses};
use serde_json::{Value, json};

use super::source;
use super::{Case, Result, RoomCase};
use crate::real_archive_item_durability::{indexed, number, rows, text};

pub(super) fn compare(catalog: &Value, case: &Case<'_>, room_case: RoomCase) -> Result {
    let sources = SourceUses::new();
    let uses = indexed(catalog, "/provenance/uses", "id")?;
    let position = |entry: &source::EntryEvidence| -> Result<(String, u64)> {
        let scope = sources.observe(SourceInput::Archive(&entry.origin), InputScope::Payload)?;
        Ok((scope.source_ref().as_str().to_owned(), entry.ordinal as u64))
    };
    let stream = position(&case.witness.stream)?;
    let witness = &case.witness;
    let row = matching_row(catalog, case, &stream.0)?;
    assert!(
        text(row, "/shipGuid/value")? == case.ship.guid.to_string(),
        "ship root identity"
    );
    assert_eq!(
        number(row, "/source/classRecordIndex/value")?,
        u64::from(witness.configuration.owner),
        "original class Record"
    );
    assert!(
        text(row, "/source/streamName/value")? == witness.stream_name,
        "original stream declaration"
    );
    for (key, entry) in [
        ("container", &witness.container),
        ("descriptor", &witness.descriptor),
        ("stream", &witness.stream),
    ] {
        let (id, index) = position(entry)?;
        assert!(
            row.pointer(&format!("/source/position/value/{key}"))
                == Some(&json!({"inputRef":id,"entryIndex":index})),
            "original {key} Entry coordinates"
        );
    }
    if let Some(index) = witness.descriptor_index {
        assert_eq!(
            number(row, "/source/position/value/descriptorNodeIndex")?,
            u64::from(index),
            "original descriptor node"
        );
    }
    compare_matrices(row, witness, &uses, &stream.0)?;
    compare_coverage(catalog, case, room_case)?;
    compare_configuration(catalog, row, witness)
}

fn matching_row<'catalog>(
    catalog: &'catalog Value,
    case: &Case<'_>,
    stream: &str,
) -> Result<&'catalog Value> {
    let witness = &case.witness;
    rows(catalog, "/placements/facilityInstances/rows")?
        .iter()
        .find(|row| {
            row.pointer("/shipRecordIndex/value")
                .and_then(Value::as_u64)
                == Some(u64::from(case.ship.owner))
                && row.pointer("/source/kind").and_then(Value::as_str) == Some("interior")
                && row
                    .pointer("/source/room/owner/componentOrdinal/value")
                    .and_then(Value::as_u64)
                    == Some(case.component as u64)
                && row
                    .pointer("/source/room/objectContainerOrdinal/value")
                    .and_then(Value::as_u64)
                    == Some(case.room as u64)
                && row
                    .pointer("/source/position/value/stream/inputRef")
                    .and_then(Value::as_str)
                    == Some(stream)
                && row
                    .pointer("/source/position/value/chunkIndex")
                    .and_then(Value::as_u64)
                    == Some(witness.chunk as u64)
                && row
                    .pointer("/source/position/value/entityOrdinal")
                    .and_then(Value::as_u64)
                    == Some(witness.entity as u64)
        })
        .ok_or_else(|| "missing positive facility row for exact SOC stream and Entity slot".into())
}

fn compare_matrices(
    row: &Value,
    witness: &source::Witness,
    uses: &std::collections::HashMap<&str, &Value>,
    stream: &str,
) -> Result {
    assert_matrix(row, "/source/localTransform", &witness.local)?;
    if let Some(nested) = witness.nested.as_ref() {
        assert_matrix(row, "/source/rootRoomTransform", nested)?;
        assert!(
            number(row, "/source/position/value/containerOccurrenceOrdinal")? > 0,
            "nested occurrence has physical ancestry"
        );
    }
    let local_uses = rows(row, "/source/localTransform/sourceUsesRefs")?;
    assert!(
        !local_uses.is_empty(),
        "local matrix keeps original SOC attribute source uses"
    );
    assert!(
        local_uses.iter().all(|used| {
            used.as_str()
                .and_then(|id| uses.get(id))
                .and_then(|used| used.get("source"))
                .and_then(Value::as_str)
                == Some(stream)
        }),
        "every matrix contributor resolves to the same original SOC Entry"
    );
    Ok(())
}

fn assert_matrix(row: &Value, path: &str, expected: &Transform) -> Result {
    let cell = row.pointer(path).ok_or("matrix carrier")?;
    assert_eq!(cell["state"], "present", "{path} is present");
    let actual = rows(cell, "/value")?;
    assert_eq!(actual.len(), 3, "3 original matrix rows");
    for (actual, expected) in actual.iter().zip(expected.rows) {
        let actual = actual.as_array().ok_or("matrix row")?;
        assert_eq!(actual.len(), 4, "4 original matrix columns");
        for (actual, expected) in actual.iter().zip(expected) {
            let actual = actual.as_f64().ok_or("matrix cell")?;
            assert!(
                (actual - f64::from(expected)).abs() <= 1e-4,
                "{path} equals the original transform"
            );
        }
    }
    Ok(())
}

fn compare_configuration(catalog: &Value, row: &Value, witness: &source::Witness) -> Result {
    let links = rows(row, "/configurations")?;
    for &(position, kind) in &witness.configuration.components {
        let table = if kind == "SCItemInventoryContainerComponentParams" {
            "inventoryConfigurations"
        } else {
            "facilities"
        };
        let definition = rows(catalog, &format!("/configurations/{table}/rows"))?
            .iter()
            .find(|row| {
                row.pointer("/ownerRecordIndex/value")
                    .and_then(Value::as_u64)
                    == Some(u64::from(witness.configuration.owner))
                    && row
                        .pointer("/componentOrdinal/value")
                        .and_then(Value::as_u64)
                        == Some(position as u64)
            })
            .ok_or("source facility definition missing from selected Record scope")?;
        assert!(
            text(definition, "/ownerGuid/value")? == witness.class.to_string(),
            "definition joins original SOC class GUID"
        );
        assert!(
            links
                .iter()
                .any(|link| link.pointer("/owner/ownerRecordIndex/value")
                    == definition.pointer("/ownerRecordIndex/value")
                    && link.pointer("/owner/componentOrdinal/value")
                        == definition.pointer("/componentOrdinal/value")),
            "physical component is included in this facility instance"
        );
    }
    Ok(())
}

fn compare_coverage(catalog: &Value, case: &Case<'_>, room_case: RoomCase) -> Result {
    let coverage: Vec<_> = rows(catalog, "/placements/facilityInteriorCoverage")?
        .iter()
        .filter(|row| {
            row.pointer("/room/owner/ownerRecordIndex/value")
                .and_then(Value::as_u64)
                == Some(u64::from(case.ship.owner))
        })
        .collect();
    if case.nested_coverage {
        assert!(
            coverage.iter().any(|row| {
                rows(row, "/containers").is_ok_and(|nodes| {
                    nodes.iter().any(|node| {
                        node.get("parentOrdinal")
                            .is_some_and(|value| !value.is_null())
                            && node.pointer("/child/name/state").and_then(Value::as_str)
                                == Some("present")
                    })
                })
            }),
            "the same physical ship has positive authored nested-room coverage"
        );
    } else {
        assert!(
            coverage.len() >= case.ship.rooms.len(),
            "all original room declaration slots on the selected ship are covered"
        );
    }
    if matches!(room_case, RoomCase::Multi) {
        let mut opened = Vec::new();
        for row in &coverage {
            for node in rows(row, "/containers")? {
                let physical = node
                    .pointer("/container/value")
                    .ok_or("original opened room")?;
                if !opened.contains(&physical) {
                    opened.push(physical);
                }
            }
        }
        assert!(
            opened.len() > 1,
            "the selected multi-room ship has multiple physical rooms"
        );
    }
    Ok(())
}
