// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceValue;
use serde_json::{Map, Value, json};

use super::source_values::{present, source};

fn header(source_type: &str, folder: &str) -> Map<String, Value> {
    let path = format!("libs/foundry/records/{folder}/fixture");
    [
        (
            "guid",
            present(json!("a8000000-0000-4000-8000-000000000001")),
        ),
        (
            "recordName",
            present(json!(format!("{source_type}.Fixture"))),
        ),
        ("className", present(json!("Fixture"))),
        ("recordPath", present(json!(path))),
        ("recordIndex", present(json!(0))),
        ("sourceType", present(json!(source_type))),
        ("fileName", present(json!(format!("{path}.xml")))),
        ("metadataTag", source(SourceValue::Absent)),
        ("fields", json!({"roots": [], "values": []})),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_owned(), value))
    .collect()
}

pub(super) fn refining() -> Value {
    let mut row = header("RefiningProcess", "refiningprocess");
    row.insert("processName".to_owned(), source(SourceValue::Absent));
    row.insert("refiningSpeed".to_owned(), present(json!("Fast")));
    row.insert("refiningQuality".to_owned(), present(json!("Careful")));
    json!({"availability": "available", "rows": [row]})
}

pub(super) fn globals() -> Value {
    let mut row = header("MiningGlobalParams", "mining/miningglobalparams");
    for name in [
        "absorbableVolumeThreshold",
        "cSCUPerVolume",
        "childRockInactivityLifetime",
        "childRockInvulnerabilityTime",
        "childRockLifeTimer",
        "childRockZeroGDamping",
        "compositionMassMultiplier",
        "controlledBreakingDecayRate",
        "controlledBreakingFillRate",
        "controlledBreakingFillRateDanger",
        "dangerBreakingDecayRate",
        "dangerBreakingFillRate",
        "dangerBreakingFillRateExponent",
        "dangerToGadgetDamage",
        "decayPerMass",
        "defaultMass",
        "gadgetDestroyThreshold",
        "gadgetDetachThreshold",
        "modifierPersistenceTime",
        "optimalWindowFactor",
        "optimalWindowMaxSize",
        "optimalWindowSize",
        "optimalWindowThinnessCurveFactor",
        "powerCapacityPerMass",
        "resistanceCurveFactor",
        "terrainFactorStaticThreshold",
        "hitConsistencyParams",
        "mineableInstabilityParams",
        "mineableExplosionParams",
        "wasteResourceType",
    ] {
        row.insert(name.to_owned(), source(SourceValue::Absent));
    }
    json!({"availability": "available", "rows": [row]})
}
