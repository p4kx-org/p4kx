// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! One loadout with a nested port.

use serde_json::json;

use p4kx_extraction::SourceValue;

use super::source_values::{present, source};

pub(super) fn loadouts() -> serde_json::Value {
    let gun = json!({
        "name": present(json!("hardpoint_gun_left")), "source": "loadout",
        "minSize": present(json!(1)), "maxSize": present(json!(3)),
        "types": present(json!([{
            "type": present(json!("WeaponGun")),
            "subTypes": present(json!([present(json!("Gun"))]))
        }])),
        "flags": present(json!(["uneditable"])), "portTags": present(json!(["Gun"])),
        "requiredTags": present(json!(["Mount"])),
        "displayNameKey": source(SourceValue::Absent), "visibility": present(json!("visible")),
        "itemGuid": present(json!("30000000-0000-4000-8000-000000000001")),
        "itemClassName": present(json!("POWR_ACAS_S01")), "dockedVehicle": false, "unresolved": null,
        "sourceConstraints": {"portDefinition": null, "attachedItem": null},
        "ports": []
    });
    json!([{
        "entityGuid": present(json!("90000000-0000-4000-8000-000000000001")),
        "entityKind": present(json!("ship")),
        "sourceRecordIndex": present(json!(0)),
        "issues": ["vehicle_implementation"],
        "ports": [{
            "name": present(json!("hardpoint_turret")), "source": "vehicle",
            "minSize": source(SourceValue::Absent), "maxSize": source(SourceValue::Absent),
            "types": present(json!([])), "flags": source(SourceValue::Absent), "portTags": source(SourceValue::Absent),
            "requiredTags": source(SourceValue::Absent),
            "displayNameKey": source(SourceValue::Absent), "visibility": present(json!("unspecified")),
            "itemGuid": source(SourceValue::Absent), "itemClassName": source(SourceValue::Absent), "dockedVehicle": false,
            "unresolved": null,
            "sourceConstraints": {"portDefinition": null, "attachedItem": null},
            "ports": [gun]
        }]
    }])
}
