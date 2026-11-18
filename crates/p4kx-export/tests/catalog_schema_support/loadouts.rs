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
        "types": [{"type": "WeaponGun", "subTypes": ["Gun"]}],
        "flags": ["uneditable"], "portTags": ["Gun"],
        "requiredTags": ["Mount"],
        "displayNameKey": null, "visibility": "visible",
        "itemGuid": "30000000-0000-4000-8000-000000000001",
        "itemClassName": "POWR_ACAS_S01", "dockedVehicle": false, "unresolved": null,
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
            "types": [], "flags": [], "portTags": [],
            "requiredTags": [],
            "displayNameKey": null, "visibility": "unspecified",
            "itemGuid": null, "itemClassName": null, "dockedVehicle": false,
            "unresolved": null,
            "sourceConstraints": {"portDefinition": null, "attachedItem": null},
            "ports": [gun]
        }]
    }])
}
