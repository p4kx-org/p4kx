// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::loot_v3_graph as raw;
use crate::mission_template_graph::{self as source, Property};
use crate::mission_values_fixture::{Owner, TestResult};

const TARGET: &str = "79000000-0000-4000-8000-000000000001";
const MISSING: &str = "79000000-0000-4000-8000-000000000002";
const TARGET_INDEX: u32 = 2;
const AUTO_TARGET: &str = "79000000-0000-4000-8000-000000000007";
const AUTO_TARGET_INDEX: u32 = 3;

pub(super) fn variants(catalog: &Value, owner: Owner) -> TestResult {
    let base = owner.property(catalog, "Base")?;
    base.kind("autoSpawns")?;
    raw::structure(base.node, "MissionPropertyValue_AutoSpawns")?;
    let options = base.field(base.node, "searchOptions")?;
    assert!(
        raw::structure(options, "SpawnClosetSearchOptionsBase")?.is_empty(),
        "the schema-only base remains its exact concrete source variant"
    );
    source::reference(
        base.field(base.node, "autoSpawns")?,
        AUTO_TARGET,
        Some(AUTO_TARGET_INDEX),
    )?;
    let entity = owner.property(catalog, "Entity")?;
    let options = entity.field(entity.node, "searchOptions")?;
    raw::structure(options, "SpawnClosetSearchOptions_EntityDeclaration")?;
    source::reference(entity.field(options, "entityDeclaration")?, MISSING, None)?;
    let tags = owner.property(catalog, "Tags")?;
    let options = tags.field(tags.node, "searchOptions")?;
    raw::structure(options, "SpawnClosetSearchOptions_Tags")?;
    let positive = tags.field(tags.field(options, "closetPositiveTags")?, "tags")?;
    assert_eq!(
        raw::elements(positive)?.len(),
        4,
        "all positive slots, repetitions and nulls survive"
    );
    for index in [0, 3] {
        source::reference(tags.element(positive, index)?, TARGET, Some(TARGET_INDEX))?;
    }
    source::reference(tags.element(positive, 1)?, MISSING, None)?;
    raw::source_null(tags.element(positive, 2)?);
    assert_ne!(
        tags.element(positive, 0)?.get("sourceOrdinal"),
        tags.element(positive, 3)?.get("sourceOrdinal"),
        "repeated identities remain distinct actual original occurrences"
    );
    let negative = tags.field(tags.field(options, "closetNegativeTags")?, "tags")?;
    assert!(
        raw::elements(negative)?.is_empty(),
        "the explicit empty negative list remains empty"
    );
    Ok(())
}

pub(super) fn absent(catalog: &Value) -> TestResult {
    let auto = Property::new(catalog, "Auto")?;
    auto.kind("autoSpawns")?;
    raw::source_null(auto.field(auto.node, "autoSpawns")?);
    raw::source_null(auto.field(auto.node, "searchOptions")?);
    let case = Property::new(catalog, "Case")?;
    case.kind("investigation")?;
    assert!(
        raw::elements(case.field(case.node, "suspects")?)?.is_empty(),
        "the declared empty Class array remains present and empty"
    );
    let race = Property::new(catalog, "Race")?;
    race.kind("timeTrialRace")?;
    assert!(
        raw::elements(race.field(race.node, "targetSplits")?)?.is_empty(),
        "the declared empty split array remains present and empty"
    );
    Ok(())
}

pub(super) fn identities(catalog: &Value, owner: Owner) -> TestResult {
    let property = owner.property(catalog, "Raw")?;
    property.kind("autoSpawns")?;
    let target = property.field(property.node, "autoSpawns")?;
    source::reference(target, AUTO_TARGET, Some(AUTO_TARGET_INDEX))?;
    assert_eq!(
        raw::present(
            source::scalar(target, "recordReference")?
                .get("targetType")
                .ok_or("actual independently sourced target type")?
        )?,
        &json!("AutoSpawns"),
        "technical target metadata is retained"
    );
    let options = property.field(property.node, "searchOptions")?;
    let positive = property.field(property.field(options, "closetPositiveTags")?, "tags")?;
    assert_eq!(
        raw::elements(positive)?.len(),
        4,
        "the original positive list length remains exact"
    );
    source::reference(property.element(positive, 0)?, TARGET, Some(TARGET_INDEX))?;
    source::reference(property.element(positive, 1)?, MISSING, None)?;
    raw::source_null(property.element(positive, 2)?);
    source::reference(property.element(positive, 3)?, TARGET, Some(TARGET_INDEX))?;
    let negative = property.field(property.field(options, "closetNegativeTags")?, "tags")?;
    assert_eq!(
        raw::elements(negative)?.len(),
        2,
        "the populated negative list keeps independent order"
    );
    source::reference(property.element(negative, 0)?, MISSING, None)?;
    source::reference(property.element(negative, 1)?, TARGET, Some(TARGET_INDEX))?;
    Ok(())
}
