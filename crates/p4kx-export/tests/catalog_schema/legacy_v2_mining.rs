// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use serde_json::json;

use super::text_routes::{TextDomain, TextPresence, check_text_route};
use super::{Inventory, Result, family_fields, released_schema, schema_at};

const REFINING: &str = "/definitions/refiningProcesses/rows/[]";
const GLOBAL: &str = "/configurations/miningGlobal/rows/[]";

#[test]
fn every_released_refining_and_global_field_has_one_canonical_destination() -> Result {
    let old = released_schema()?;
    let inventory = Inventory::collect(&old)?;
    let current = p4kx_export::catalog_schema();
    for (family, mappings) in [
        ("refiningProcesses", refining()),
        ("miningGlobalParams", globals()),
    ] {
        assert!(
            current["properties"].get(family).is_none(),
            "no old root dataset survives"
        );
        let classified = mappings
            .iter()
            .map(|(old, _)| old.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            classified.len(),
            mappings.len(),
            "each old path is classified exactly once"
        );
        assert_eq!(
            family_fields(&inventory, family)?,
            classified,
            "no released original datum is silently omitted"
        );
        for (old, destination) in mappings {
            schema_at(&current, &destination)
                .map_err(|error| format!("{family}{old} -> {destination}: {error}"))?;
        }
    }
    check_text_route(
        &current,
        REFINING,
        "processName",
        TextDomain::GameText,
        TextPresence::Direct,
    )?;
    Ok(())
}

#[test]
fn an_added_historical_mining_field_requires_an_explicit_disposition() -> Result {
    let mut old = released_schema()?;
    old["$defs"]["MiningGlobalParams"]["properties"]["unclassifiedOriginal"] =
        json!({"type":"number"});
    let inventory = Inventory::collect(&old)?;
    let known = globals()
        .into_iter()
        .map(|(old, _)| old)
        .collect::<BTreeSet<_>>();
    let extra = family_fields(&inventory, "miningGlobalParams")?
        .into_iter()
        .filter(|path| !known.contains(*path))
        .collect::<Vec<_>>();
    assert_eq!(
        extra,
        ["/unclassifiedOriginal"],
        "the inventory exposes the precise missing route"
    );
    Ok(())
}

fn headers(base: &str) -> Vec<(String, String)> {
    ["guid", "className", "recordPath"]
        .into_iter()
        .map(|field| (format!("/{field}"), format!("{base}/{field}/value")))
        .collect()
}

fn refining() -> Vec<(String, String)> {
    let mut paths = headers(REFINING);
    for field in ["processName", "refiningSpeed", "refiningQuality"] {
        paths.push((format!("/{field}"), format!("{REFINING}/{field}/value")));
    }
    paths.push((
        "/processName/key".to_owned(),
        "/texts/gameTexts/[]/key/value".to_owned(),
    ));
    for language in ["en", "de"] {
        paths.push((
            format!("/processName/{language}"),
            "/texts/gameTexts/[]/translations/value/*/displayText/value".to_owned(),
        ));
    }
    paths
}

const ROOT_NUMBERS: [(&str, &str); 25] = [
    ("absorbableVolumeThreshold", "absorbableVolumeThreshold"),
    ("cScuPerVolume", "cSCUPerVolume"),
    ("childRockInactivityLifetime", "childRockInactivityLifetime"),
    (
        "childRockInvulnerabilityTime",
        "childRockInvulnerabilityTime",
    ),
    ("childRockLifeTimer", "childRockLifeTimer"),
    ("childRockZeroGDamping", "childRockZeroGDamping"),
    ("controlledBreakingDecayRate", "controlledBreakingDecayRate"),
    ("controlledBreakingFillRate", "controlledBreakingFillRate"),
    (
        "controlledBreakingFillRateDanger",
        "controlledBreakingFillRateDanger",
    ),
    ("dangerBreakingDecayRate", "dangerBreakingDecayRate"),
    ("dangerBreakingFillRate", "dangerBreakingFillRate"),
    (
        "dangerBreakingFillRateExponent",
        "dangerBreakingFillRateExponent",
    ),
    ("dangerToGadgetDamage", "dangerToGadgetDamage"),
    ("decayPerMass", "decayPerMass"),
    ("defaultMass", "defaultMass"),
    ("gadgetDestroyThreshold", "gadgetDestroyThreshold"),
    ("gadgetDetachThreshold", "gadgetDetachThreshold"),
    ("modifierPersistenceTime", "modifierPersistenceTime"),
    ("optimalWindowFactor", "optimalWindowFactor"),
    ("optimalWindowMaxSize", "optimalWindowMaxSize"),
    ("optimalWindowSize", "optimalWindowSize"),
    (
        "optimalWindowThinnessCurveFactor",
        "optimalWindowThinnessCurveFactor",
    ),
    ("powerCapacityPerMass", "powerCapacityPerMass"),
    ("resistanceCurveFactor", "resistanceCurveFactor"),
    (
        "terrainFactorStaticThreshold",
        "terrainFactorStaticThreshold",
    ),
];

fn globals() -> Vec<(String, String)> {
    let mut paths = headers(GLOBAL);
    for (old, field) in ROOT_NUMBERS {
        paths.push((format!("/{old}"), format!("{GLOBAL}/{field}/value")));
    }
    for (old, new) in [
        ("hitConsistency", "hitConsistencyParams"),
        ("instability", "mineableInstabilityParams"),
        ("explosion", "mineableExplosionParams"),
    ] {
        paths.push((format!("/{old}"), format!("{GLOBAL}/{new}/value")));
    }
    nested(
        &mut paths,
        "hitConsistency",
        "hitConsistencyParams",
        &[
            ("extractionMagnitude", "extractionMagnitude"),
            ("hitHistoryWindow", "hitHistoryWindow"),
            ("maxEffectOnInstability", "maxEffectOnInstability"),
            ("minDeviation", "minDeviation"),
            ("standardDeviationMultiplier", "standardDeviationMultiplier"),
            ("timeExponent", "timeExponent"),
        ],
    );
    nested(
        &mut paths,
        "instability",
        "mineableInstabilityParams",
        &[
            ("curveFactor", "instabilityCurveFactor"),
            ("wavePeriod", "instabilityWavePeriod"),
            ("waveVariance", "instabilityWaveVariance"),
        ],
    );
    nested(
        &mut paths,
        "explosion",
        "mineableExplosionParams",
        &[
            ("dangerPoolFactor", "dangerPoolFactor"),
            ("defaultVolume", "defaultVolume"),
        ],
    );
    paths.push((
        "/wasteResourceTypeGuid".to_owned(),
        format!("{GLOBAL}/wasteResourceType/value/guid/value"),
    ));
    paths
}

fn nested(paths: &mut Vec<(String, String)>, old: &str, new: &str, fields: &[(&str, &str)]) {
    paths.extend(fields.iter().map(|(before, after)| {
        (
            format!("/{old}/{before}"),
            format!("{GLOBAL}/{new}/value/{after}/value"),
        )
    }));
}
