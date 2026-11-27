// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::loot::{LootDefinitionSnapshot, SubHarvestableMultiConfig};
use p4kx_export::catalog_schema;
use p4kx_extraction::{FieldPath, SourceInventory, SourceLocation, SourceUseRole};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::{DEPENDENCY, EARLIER, LATER, TestResult};

const ENTITY: &str = "EntityClassDefinition";
const MINEABLE: &str = "MineableParams";
const TARGET: &str = "FixtureMiningTarget";
const SLOT: &str = "/taggedConfigs/0/subHarvestables/0";
const FALLBACK_INDEX: u64 = 5;
const DIRECT_INDEX: u64 = 6;
const COMPOSITION: u128 = 70;
const GLOBAL: u128 = 71;
pub(super) const FALLBACK_ENTITY: u128 = 60;
pub(super) const DIRECT_ENTITY: u128 = 61;

pub(super) fn add_dependencies(data: DataForgeBuilder) -> DataForgeBuilder {
    let fallback = TestRecord::new(
        FALLBACK_ENTITY,
        ENTITY,
        "EntityClassDefinition.Fallback",
        "synthetic/fallback.xml",
    )
    .value(
        "Components",
        TestValue::Array(vec![
            TestValue::Null,
            TestValue::Struct(
                TestInstance::new(MINEABLE)
                    .value("composition", TestValue::Reference(COMPOSITION))
                    .value("globalParams", TestValue::Reference(GLOBAL)),
            ),
            TestValue::Struct(
                TestInstance::new(MINEABLE).value("composition", TestValue::Reference(GLOBAL)),
            ),
        ]),
    );
    data.structure(
        TestStruct::new(ENTITY).array("Components", TestType::StrongPointer(MINEABLE.to_owned())),
    )
    .structure(
        TestStruct::new(MINEABLE)
            .field("composition", TestType::Reference)
            .field("globalParams", TestType::Reference),
    )
    .structure(TestStruct::new(TARGET).field("fixture", TestType::Bool))
    .record(fallback)
    .record(TestRecord::new(
        DIRECT_ENTITY,
        ENTITY,
        "EntityClassDefinition.Direct",
        "synthetic/direct.xml",
    ))
    .record(TestRecord::new(
        COMPOSITION,
        TARGET,
        "FixtureMiningTarget.Composition",
        "synthetic/composition.xml",
    ))
    .record(TestRecord::new(
        GLOBAL,
        TARGET,
        "FixtureMiningTarget.Global",
        "synthetic/global.xml",
    ))
}

pub(super) fn assert_dependency_origins(snapshot: &LootDefinitionSnapshot<'_>) -> TestResult {
    let [fallback, direct] = snapshot.sub_harvestable_multi_configs.as_slice() else {
        return Err("two admitted slot presets".into());
    };
    assert_link_values(fallback, direct)?;
    assert_entity_sources(snapshot, fallback, direct)?;
    assert_mining_sources(snapshot, fallback, direct)
}

fn assert_link_values(
    fallback: &SubHarvestableMultiConfig<'_>,
    direct: &SubHarvestableMultiConfig<'_>,
) -> TestResult {
    let link = |row: &SubHarvestableMultiConfig<'_>| {
        let tagged = row.tagged_configs.first().ok_or("tagged config")?;
        let slot = tagged.sub_harvestables.first().ok_or("slot")?;
        let link = slot.harvestable.as_ref().ok_or("harvestable")?;
        Ok::<_, &str>((
            link.entity_guid,
            link.mineable_composition_guid,
            link.mining_global_params_guid,
        ))
    };
    let (fallback_entity, composition, global_params) = link(fallback)?;
    let (direct_entity, direct_composition, direct_global_params) = link(direct)?;
    assert!(
        fallback_entity.is_some() && direct_entity.is_some(),
        "both paths select an Entity"
    );
    assert_ne!(
        fallback_entity, direct_entity,
        "the direct override wins over the same preset"
    );
    assert!(
        composition.is_some() && global_params.is_some(),
        "the first MineableParams supplies both fields"
    );
    assert_ne!(
        composition, global_params,
        "the selected dependency references are distinct"
    );
    assert_eq!(
        (direct_composition, direct_global_params),
        (None, None),
        "the direct Entity provides neither mining reference"
    );

    Ok(())
}

fn assert_entity_sources(
    snapshot: &LootDefinitionSnapshot<'_>,
    fallback: &SubHarvestableMultiConfig<'_>,
    direct: &SubHarvestableMultiConfig<'_>,
) -> TestResult {
    let fallback_use = cell_uses(snapshot, fallback, "entityGuid", true)?;
    let direct_use = cell_uses(snapshot, direct, "entityGuid", true)?;
    assert!(
        has(
            &fallback_use,
            [u64::from(DEPENDENCY), 1],
            SourceUseRole::Payload
        ),
        "fallback reads the selected preset entityClass"
    );
    assert!(
        has(&direct_use, [u64::from(LATER), 12], SourceUseRole::Payload),
        "the direct override is the actual local reference"
    );
    assert!(
        !has(
            &direct_use,
            [u64::from(DEPENDENCY), 1],
            SourceUseRole::Payload
        ),
        "the direct origin excludes the preset fallback"
    );

    Ok(())
}

fn assert_mining_sources(
    snapshot: &LootDefinitionSnapshot<'_>,
    fallback: &SubHarvestableMultiConfig<'_>,
    direct: &SubHarvestableMultiConfig<'_>,
) -> TestResult {
    for (leaf, ordinal) in [
        ("mineableCompositionGuid", 3),
        ("miningGlobalParamsGuid", 4),
    ] {
        let fallback_use = cell_uses(snapshot, fallback, leaf, true)?;
        assert!(
            has(&fallback_use, [FALLBACK_INDEX, 1], SourceUseRole::Selector),
            "{leaf} retains the earlier null component in the actual selector prefix"
        );
        assert!(
            has(
                &fallback_use,
                [FALLBACK_INDEX, ordinal],
                SourceUseRole::Payload
            ),
            "{leaf} uses the leaf from the first matching component"
        );
        assert!(
            !has(
                &fallback_use,
                [FALLBACK_INDEX, ordinal + 3],
                SourceUseRole::Payload
            ),
            "{leaf} excludes the later matching component"
        );
        let direct_use = cell_uses(snapshot, direct, leaf, true)?;
        assert!(
            has(&direct_use, [DIRECT_INDEX, 0], SourceUseRole::Selector),
            "the direct Entity's empty component list proves absence"
        );
    }
    Ok(())
}

pub(super) fn assert_preset_failure(
    snapshot: &LootDefinitionSnapshot<'_>,
    direct: bool,
) -> TestResult {
    let row = snapshot
        .sub_harvestable_multi_configs
        .first()
        .ok_or("present slot row")?;
    let attempts: Vec<_> = snapshot
        .source_inventory
        .iter_uses()
        .filter(|source| source.role() == SourceUseRole::Attempted)
        .collect();
    assert!(
        attempts.iter().any(|source| matches!(
            source.location(),
            SourceLocation::Subresource { namespace, indices, field }
                if namespace == "dataforgeRecordBody" && field == "body"
                && indices.as_slice() == [u64::from(DEPENDENCY)]
        )),
        "a failed preset body remains an observed attempt"
    );
    let _ = cell_uses(snapshot, row, "presetClassName", false)?;
    let _ = cell_uses(snapshot, row, "entityGuid", direct)?;
    let _ = cell_uses(snapshot, row, "mineableCompositionGuid", direct)?;
    let _ = cell_uses(snapshot, row, "miningGlobalParamsGuid", direct)?;
    Ok(())
}

pub(super) fn assert_schema() -> TestResult {
    let schema = catalog_schema();
    let row = schema
        .pointer("/$defs/SubHarvestableMultiConfig")
        .ok_or("v3 slot-preset schema")?;
    let required = row
        .get("required")
        .and_then(serde_json::Value::as_array)
        .ok_or("required row fields")?;
    assert!(
        required.iter().any(|name| name == "fieldOrigins"),
        "the row requires the immediate origin map"
    );
    let shape = schema
        .pointer("/$defs/FieldOrigin")
        .ok_or("canonical field-origin schema")?;
    for field in [
        "containsEncryptedSource",
        "sourceSetComplete",
        "sourceUsesRefs",
    ] {
        assert!(
            shape
                .get("properties")
                .and_then(|properties| properties.get(field))
                .is_some(),
            "the v3 map uses the canonical field-origin schema for {field}"
        );
    }
    Ok(())
}

pub(super) fn assert_normal_source_closure(
    snapshot: &LootDefinitionSnapshot<'_>,
    normal: &SourceInventory,
) -> TestResult {
    for row in &snapshot.sub_harvestable_multi_configs {
        for (path, origin) in row.field_origins.iter() {
            for reference in origin.source_uses_refs() {
                let wire = normal
                    .iter_uses()
                    .find(|candidate| candidate.id() == reference)
                    .ok_or("Normal field origin resolves in its original source inventory")?;
                let sdk = snapshot
                    .source_inventory
                    .iter_uses()
                    .find(|candidate| candidate.id() == reference)
                    .ok_or("SDK field origin resolves in its original source inventory")?;
                assert!(
                    wire.source() == sdk.source()
                        && wire.scope() == sdk.scope()
                        && wire.role() == sdk.role()
                        && wire.location() == sdk.location()
                        && wire.effective_encryption() == sdk.effective_encryption(),
                    "{path:?} binds the same physical contributor in Normal and SDK"
                );
            }
        }
    }
    Ok(())
}

fn cell_uses<'snapshot>(
    snapshot: &'snapshot LootDefinitionSnapshot<'_>,
    row: &SubHarvestableMultiConfig<'_>,
    leaf: &str,
    complete: bool,
) -> TestResult<Vec<&'snapshot p4kx_extraction::SourceUseDescription>> {
    let pointer = FieldPath::parse(&format!("{SLOT}/harvestable/{leaf}"))?;
    let origin = row
        .field_origins
        .get(&pointer)
        .ok_or("immediate dependency origin")?;
    assert_eq!(
        origin.source_set_complete(),
        complete,
        "{leaf} retains the dependency read state"
    );
    origin
        .source_uses_refs()
        .iter()
        .map(|reference| {
            let source = snapshot
                .source_inventory
                .iter_uses()
                .find(|source| source.id() == reference)
                .ok_or("the origin reference resolves in the scoped inventory")?;
            assert_ne!(
                source.role(),
                SourceUseRole::Attempted,
                "failed payloads remain outside the contributing set"
            );
            Ok(source)
        })
        .collect()
}

fn has(
    uses: &[&p4kx_extraction::SourceUseDescription],
    coordinate: [u64; 2],
    role: SourceUseRole,
) -> bool {
    uses.iter().any(|source| source.role() == role && matches!(
        source.location(),
        SourceLocation::Subresource { namespace, indices, field }
            if namespace == "dataforgeValue" && indices.as_slice() == coordinate && field == "$value"
    ))
}

pub(super) fn assert_local_origins(snapshot: &LootDefinitionSnapshot<'_>) -> TestResult {
    use SourceUseRole::{Payload, Selector};

    let row = snapshot
        .sub_harvestable_multi_configs
        .first()
        .ok_or("selected slot preset")?;
    let [tagged] = row.tagged_configs.as_slice() else {
        return Err("one tagged config".into());
    };
    let [slot] = tagged.sub_harvestables.as_slice() else {
        return Err("one retained original slot".into());
    };
    assert_eq!(
        (slot.setup_guid, slot.respawn_time_multiplier),
        (None, None),
        "an explicitly null Reference and an undeclared field both project to null"
    );

    for (leaf, ordinal, field, role) in [
        ("relativeProbability", 10, "$value", Payload),
        ("relativeProbability", 7, "$value", Selector),
        ("relativeProbability", 8, "$value", Selector),
        ("setupGuid", 11, "$value", Selector),
        ("respawnTimeMultiplier", 8, "$fields", Selector),
        ("harvestable/presetGuid", 9, "$value", Payload),
    ] {
        let path = FieldPath::parse(&format!("{SLOT}/{leaf}"))?;
        let origin = row.field_origins.get(&path).ok_or("local field origin")?;
        assert!(
            origin.source_set_complete(),
            "{leaf} has complete local inputs"
        );
        let mut found = false;
        for reference in origin.source_uses_refs() {
            let source = snapshot
                .source_inventory
                .iter_uses()
                .find(|source| source.id() == reference)
                .ok_or("origin resolves in the scoped inventory")?;
            found |= source.role() == role
                && matches!(
                    source.location(),
                    SourceLocation::Subresource { namespace, indices, field: actual }
                        if namespace == "dataforgeValue"
                            && indices.as_slice() == [u64::from(EARLIER), ordinal]
                            && actual == field
                );
        }
        assert!(
            found,
            "{leaf} observes {field} of original ordinal {ordinal}"
        );
    }
    Ok(())
}
