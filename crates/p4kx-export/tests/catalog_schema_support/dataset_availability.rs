// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::absent_table;

fn add_empty_definitions(catalog: &mut Value) {
    for kind in [
        "factionIdentities",
        "securityNetworkManifests",
        "missionModuleDeclarations",
        "missionItems",
        "contractScopes",
        "locationResourceSlots",
        "locationEntityDeclarations",
        "particleLibraries",
        "flareLibraries",
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

pub(super) fn add_defaults(catalog: &mut Value) {
    if let Some(completeness) = catalog
        .get_mut("completeness")
        .and_then(Value::as_object_mut)
    {
        for name in ["cargoLoadingGlobals", "shopCommodityGlobals"] {
            completeness.insert(name.to_owned(), json!({"status": "complete"}));
        }
    }
    add_empty_definitions(catalog);
    if let Some(configurations) = catalog
        .get_mut("configurations")
        .and_then(Value::as_object_mut)
    {
        for name in [
            "radarOptions",
            "flightControllers",
            "ifcs",
            "thrusters",
            "cargoLoadingGlobals",
            "shopCommodityGlobals",
        ] {
            configurations.insert(name.to_owned(), absent_table());
        }
    }
}
