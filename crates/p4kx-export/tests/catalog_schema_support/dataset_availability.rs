// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::absent_table;

pub(super) fn add_empty_definitions(catalog: &mut Value) {
    for kind in [
        "factionIdentities",
        "securityNetworkManifests",
        "missionModuleDeclarations",
        "missionItems",
        "contractScopes",
        "locationResourceSlots",
        "particleLibraries",
        "planetResources",
        "creatures",
        "climateAssets",
        "geologyAssets",
        "ecosystems",
        "geologySets",
        "geologyTextureSets",
        "soils",
        "soilTextureSets",
        "shadingClasses",
        "proceduralPlanetAudio",
        "resourceNetworkGlobals",
    ] {
        for (section, value) in [
            ("completeness", json!({"status": "complete"})),
            ("definitions", absent_table()),
        ] {
            if let Some(object) = catalog.get_mut(section).and_then(Value::as_object_mut) {
                object.insert(kind.to_owned(), value);
            }
        }
    }
}
