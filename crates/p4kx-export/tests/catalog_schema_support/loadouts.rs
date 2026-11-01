// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! One loadout with a nested port.

use serde_json::json;

pub(super) fn loadouts() -> serde_json::Value {
    let gun = json!({
        "name": "hardpoint_gun_left", "source": "loadout",
        "minSize": 1, "maxSize": 3,
        "types": [{"type": "WeaponGun", "subTypes": ["Gun"]}],
        "flags": ["uneditable"], "portTags": ["Gun"],
        "requiredTags": ["Mount"],
        "displayNameKey": null, "visibility": "visible",
        "itemGuid": "30000000-0000-4000-8000-000000000001",
        "itemClassName": "POWR_ACAS_S01", "dockedVehicle": false, "unresolved": null,
        "ports": []
    });
    json!([{
        "entityGuid": "90000000-0000-4000-8000-000000000001",
        "entityKind": "ship",
        "issues": ["vehicle_implementation"],
        "ports": [{
            "name": "hardpoint_turret", "source": "vehicle",
            "minSize": null, "maxSize": null,
            "types": [], "flags": [], "portTags": [],
            "requiredTags": [],
            "displayNameKey": null, "visibility": "unspecified",
            "itemGuid": null, "itemClassName": null, "dockedVehicle": false,
            "unresolved": null,
            "ports": [gun]
        }]
    }])
}
