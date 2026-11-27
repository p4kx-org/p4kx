// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::de::Error;
use serde_json::Value;

pub(super) fn flight_includes<E: Error>(ship: u32, row: &Value) -> Result<bool, E> {
    Ok(physical_ship_index::<E>(row)? == ship)
}

pub(super) fn interior_includes<E: Error>(ship: u32, row: &Value) -> Result<bool, E> {
    Ok(interior_room_root::<E>(row)? == ship)
}

pub(super) fn interior_placement_includes<E: Error>(ship: u32, row: &Value) -> Result<bool, E> {
    Ok(interior_placement_root::<E>(row)? == Some(ship))
}

pub(in super::super) fn interior_placement_root<E: Error>(row: &Value) -> Result<Option<u32>, E> {
    let source = row
        .get("source")
        .ok_or_else(|| E::custom("a Facility placement has no physical source"))?;
    match source.get("kind").and_then(Value::as_str) {
        Some("loadout") => return Ok(None),
        Some("interior") => {}
        _ => return Err(E::custom("a Facility placement has an invalid source kind")),
    }
    let index = physical_ship_index::<E>(row)?;
    if interior_room_root::<E>(source)? != index {
        return Err(E::custom(
            "a Facility Interior placement disagrees with its physical room root",
        ));
    }
    Ok(Some(index))
}

pub(in super::super) fn physical_ship_index<E: Error>(row: &Value) -> Result<u32, E> {
    let index = row
        .pointer("/shipRecordIndex/value")
        .and_then(record_index)
        .ok_or_else(|| E::custom("a Catalog row has no physical Ship Record index"))?;
    if row
        .pointer("/shipRecordIndex/state")
        .and_then(Value::as_str)
        != Some("present")
    {
        return Err(E::custom("a Catalog row has no present Ship identity"));
    }
    Ok(index)
}

pub(in super::super) fn interior_room_root<E: Error>(row: &Value) -> Result<u32, E> {
    let attachment = row
        .pointer("/room/loadoutPosition")
        .ok_or_else(|| E::custom("a Facility Interior row has no loadoutPosition selector"))?;
    if attachment.is_null() {
        let owner = row
            .pointer("/room/owner/ownerRecordIndex")
            .and_then(present)
            .and_then(record_index);
        owner.ok_or_else(|| E::custom("a direct Facility Interior row has no physical Ship Record"))
    } else {
        let position = present(attachment).ok_or_else(|| {
            E::custom("an attached Facility Interior row has no present position")
        })?;
        let root = position
            .get("loadoutRecordIndex")
            .and_then(record_index)
            .ok_or_else(|| {
                E::custom("an attached Facility Interior row has no physical Loadout root")
            })?;
        let ordinals = position
            .get("portOrdinals")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                E::custom("an attached Facility Interior row has no ordered port path")
            })?;
        if ordinals.is_empty()
            || !ordinals
                .iter()
                .all(|ordinal| record_index(ordinal).is_some())
        {
            return Err(E::custom(
                "an attached Facility Interior row has an invalid port path",
            ));
        }
        Ok(root)
    }
}

fn present(cell: &Value) -> Option<&Value> {
    if cell.get("state").and_then(Value::as_str) == Some("present") {
        cell.get("value")
    } else {
        None
    }
}

fn record_index(value: &Value) -> Option<u32> {
    u32::try_from(value.as_u64()?).ok()
}

#[cfg(test)]
mod tests {
    use serde::de::DeserializeSeed as _;
    use serde_json::{Map, Value, json};

    use super::super::{ComparedRows, RowSelection};
    use crate::real_archive_catalog_table_parity::TestResult;

    const DATASET: &str = "/placements/facilityInteriorCoverage";
    const PLACEMENTS: &str = "/placements/facilityInstances";
    const SELECTED: u32 = 2;

    fn room(owner: u32, position: Value) -> Value {
        json!({"room": declaration(owner, position)})
    }

    fn declaration(owner: u32, position: Value) -> Value {
        Value::Object(Map::from_iter([
            (
                "owner".into(),
                json!({"ownerRecordIndex": {"state": "present", "value": owner}}),
            ),
            ("loadoutPosition".into(), position),
        ]))
    }

    fn placement(ship: u32, source: Value) -> Value {
        Value::Object(Map::from_iter([
            (
                "shipRecordIndex".into(),
                json!({"state": "present", "value": ship}),
            ),
            ("source".into(), source),
        ]))
    }

    fn interior(ship: u32, owner: u32, position: Value) -> Value {
        placement(
            ship,
            json!({"kind": "interior", "room": declaration(owner, position)}),
        )
    }

    fn attached(root: u32, path: &[u32]) -> Value {
        json!({"state":"present","value":{"loadoutRecordIndex":root,"portOrdinals":path}})
    }

    #[test]
    fn selected_room_array_preserves_direct_and_attached_occurrences_in_order() -> TestResult {
        let direct = room(SELECTED, Value::Null);
        let first = room(42, attached(SELECTED, &[0]));
        let second = room(42, attached(SELECTED, &[0, 1]));
        let other = room(SELECTED, attached(9, &[0]));
        let expected = [direct.clone(), first.clone(), second.clone()];
        let seed =
            ComparedRows::bare_array(DATASET, &expected, RowSelection::InteriorShip(SELECTED));
        let json = json!([other, direct, room(1, Value::Null), first, other, second]).to_string();
        let mut decoder = serde_json::Deserializer::from_str(&json);
        assert_eq!(
            seed.deserialize(&mut decoder)?,
            json!(3),
            "the selected Ship owns its direct row and both physical port occurrences"
        );
        decoder.end()?;
        for input in [
            json!([direct, second, first]),
            json!([direct, first, other]),
            json!([direct, first, second, second]),
        ] {
            assert!(
                seed.deserialize(&mut serde_json::Deserializer::from_str(&input.to_string()))
                    .is_err(),
                "a reordered, missing or additional selected occurrence cannot pass"
            );
        }
        Ok(())
    }

    #[test]
    fn selected_facility_placements_use_the_physical_interior_root() -> TestResult {
        let direct = interior(SELECTED, SELECTED, Value::Null);
        let installed = interior(SELECTED, 42, attached(SELECTED, &[0, 1]));
        let other = interior(9, 42, attached(9, &[0]));
        let loadout = placement(SELECTED, json!({"kind": "loadout"}));
        let expected = [direct.clone(), installed.clone()];
        let seed = ComparedRows::table(
            PLACEMENTS,
            &expected,
            RowSelection::InteriorPlacement(SELECTED),
        );
        let input = json!([loadout, direct, other, installed]).to_string();
        let mut decoder = serde_json::Deserializer::from_str(&input);
        assert_eq!(
            seed.deserialize(&mut decoder)?,
            json!(2),
            "only the exact interior source and selected physical Ship are compared"
        );
        decoder.end()?;
        for invalid in [
            json!({"source": {"kind": "interior"}}),
            placement(SELECTED, json!({"kind": "unknown"})),
            interior(SELECTED, 9, Value::Null),
            interior(SELECTED, 42, attached(9, &[0])),
            interior(SELECTED, 42, attached(SELECTED, &[])),
        ] {
            let input = json!([direct, installed, invalid]).to_string();
            assert!(
                seed.deserialize(&mut serde_json::Deserializer::from_str(&input))
                    .is_err(),
                "an invalid source kind or malformed or conflicting physical root cannot be ignored"
            );
        }
        Ok(())
    }

    #[test]
    fn malformed_attached_room_cannot_fall_back_to_declaration_owner() {
        let direct = room(SELECTED, Value::Null);
        let expected = [direct.clone()];
        let seed =
            ComparedRows::bare_array(DATASET, &expected, RowSelection::InteriorShip(SELECTED));
        for invalid in [
            json!({"room":{"owner":{"ownerRecordIndex":{"state":"present","value":SELECTED}}}}),
            room(SELECTED, json!({"state":"unreadable","value":null})),
            room(
                SELECTED,
                json!({"state":"present","value":{"loadoutRecordIndex":"2","portOrdinals":[0]}}),
            ),
            room(
                SELECTED,
                json!({"state":"present","value":{"loadoutRecordIndex":SELECTED,"portOrdinals":["0"]}}),
            ),
            room(SELECTED, attached(SELECTED, &[])),
        ] {
            let input = json!([direct, invalid]).to_string();
            let error = seed
                .deserialize(&mut serde_json::Deserializer::from_str(&input))
                .expect_err("a malformed attachment must fail even after all expected rows");
            assert!(
                error.to_string().contains("Facility Interior row has"),
                "the comparison rejects the selector instead of treating it as a direct or foreign row: {error}"
            );
        }
    }
}
