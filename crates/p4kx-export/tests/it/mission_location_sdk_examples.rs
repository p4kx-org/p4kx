// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    MissionLocationTemplateSnapshot, MissionModuleInput, MissionModuleInputPayload,
};
use p4kx_extraction::{SourceValueState, SourcedValue};

use super::TestResult;

pub(super) fn limits(snapshot: &MissionLocationTemplateSnapshot<'_>) -> TestResult {
    let row = snapshot.rows.first().ok_or("template")?;
    assert_eq!(
        row.is_security_network_host().value(),
        Some(&false),
        "original false flag"
    );
    assert!(
        row.entity_cluster_member().value().is_none(),
        "no invented member reference"
    );
    let config = row.configuration().value().ok_or("configuration")?;
    let limits = config.mission_limits().value().ok_or("limits")?;
    assert_eq!(limits.len(), 2, "both physical limits");
    for (limit, maximum) in limits.iter().zip([0, -1]) {
        let limit = limit.value().ok_or("limit")?;
        assert_eq!(
            limit.max_mission_instances().value(),
            Some(&maximum),
            "authored maximum"
        );
        assert_eq!(
            limit.auto_consume_if_player_at_location().value(),
            Some(&false),
            "authored false"
        );
        assert_eq!(
            limit.tag().state(),
            SourceValueState::Null,
            "null Tag reference"
        );
    }
    let manifest = config
        .default_security_network_manifest()
        .value()
        .ok_or("manifest reference")?;
    assert!(
        manifest.resolved_index().is_none(),
        "dangling target supplies no compatible row"
    );
    assert!(
        manifest.guid().value().is_some(),
        "the original target GUID remains typed"
    );
    Ok(())
}

pub(super) fn spawn(snapshot: &MissionLocationTemplateSnapshot<'_>) -> TestResult {
    let spawn = first_spawn(snapshot)?;
    assert_eq!(
        spawn.limits().min_group_size().value(),
        Some(&-1),
        "signed limit"
    );
    let minimum = spawn
        .limits()
        .min_spawn_delay()
        .value()
        .ok_or("minimum delay")?;
    let maximum = spawn
        .limits()
        .max_spawn_delay()
        .value()
        .ok_or("maximum delay")?;
    assert_eq!(
        minimum.raw_bits(),
        (-0.0_f32).to_bits(),
        "negative zero bits"
    );
    assert_eq!(maximum.raw_bits(), 0x7fc0_4321, "original nonfinite bits");
    assert!(
        maximum.finite().is_none(),
        "nonfinite has no finite decimal projection"
    );
    let positive = spawn
        .tags()
        .positive_character_tags()
        .value()
        .ok_or("positive tags")?;
    let slots = positive.tags().value().ok_or("tag slots")?;
    let [first, null, repeated] = slots.as_slice() else {
        return Err("three original operands".into());
    };
    assert_eq!(
        null.state(),
        SourceValueState::Null,
        "null slot stays in its original position"
    );
    assert_eq!(
        first.value().ok_or("first Tag")?.guid().value(),
        repeated.value().ok_or("repeated Tag")?.guid().value(),
        "same authored target"
    );
    assert_ne!(
        first.origin().source_uses_refs(),
        repeated.origin().source_uses_refs(),
        "repeated references remain distinct source occurrences"
    );
    Ok(())
}

pub(super) fn inventory(snapshot: &MissionLocationTemplateSnapshot<'_>) -> TestResult {
    complete_spawn(snapshot)?;
    let items = first_spawn(snapshot)?
        .inventory_items()
        .value()
        .ok_or("inventory")?;
    let [first, repeated] = items.as_slice() else {
        return Err("two authored inventory rows".into());
    };
    for item in items {
        let item = item.value().ok_or("inventory row")?;
        assert_eq!(
            item.entity_class().state(),
            SourceValueState::Null,
            "original null reference"
        );
        assert_eq!(item.quantity().value(), Some(&-1), "signed quantity");
        assert_eq!(
            item.amount_inventories_to_fill().value(),
            Some(&0),
            "zero remains zero"
        );
        assert_eq!(
            item.mark_as_mission_item().value(),
            Some(&false),
            "false remains false"
        );
        assert_eq!(
            item.authored_fields().roots().len(),
            1,
            "additional original field stays typed"
        );
    }
    assert_ne!(
        first.origin().source_uses_refs(),
        repeated.origin().source_uses_refs(),
        "equal inventory contents do not merge physical occurrences"
    );
    Ok(())
}

pub(super) fn module(snapshot: &MissionLocationTemplateSnapshot<'_>) -> TestResult {
    let config = snapshot
        .rows
        .first()
        .and_then(|row| row.configuration().value())
        .ok_or("configuration")?;
    let modules = config.mission_modules().value().ok_or("modules")?;
    let module = modules
        .first()
        .and_then(|module| module.value())
        .ok_or("module")?;
    let inputs = module.mission_inputs().value().ok_or("inputs")?;
    module_inputs(inputs)?;
    let properties = module.properties().value().ok_or("properties")?;
    assert_eq!(properties.len(), 2, "both original properties");
    assert_eq!(
        properties
            .get(1)
            .ok_or("original nullable property slot")?
            .value()
            .ok_or("second property")?
            .value()
            .state(),
        SourceValueState::Null,
        "null property payload remains distinct"
    );
    Ok(())
}

fn module_inputs(inputs: &[SourcedValue<MissionModuleInput<'_>>]) -> TestResult {
    assert_eq!(
        inputs.len(),
        8,
        "all seven interpretations and one null slot"
    );
    assert_eq!(
        inputs.get(2).ok_or("original null input slot")?.state(),
        SourceValueState::Null,
        "null input stays in place"
    );
    assert!(
        matches!(
            inputs
                .get(7)
                .ok_or("original base input slot")?
                .value()
                .ok_or("base input")?
                .payload(),
            MissionModuleInputPayload::Base
        ),
        "base input remains distinct from a future authored variant"
    );
    let authored = inputs
        .get(6)
        .ok_or("original future input slot")?
        .value()
        .ok_or("future input")?;
    assert!(
        matches!(authored.payload(), MissionModuleInputPayload::Authored),
        "future concrete variant"
    );
    assert_eq!(
        authored.authored_fields().roots().len(),
        1,
        "future value retained in the arena"
    );
    super::arena::assert_int32(authored.authored_fields(), 4)?;
    Ok(())
}

pub(super) fn classification(snapshot: &MissionLocationTemplateSnapshot<'_>) -> TestResult {
    let [complete, sparse] = snapshot.rows.as_slice() else {
        return Err("two classification rows".into());
    };
    let complete = complete.classification().value().ok_or("classification")?;
    for reference in [
        complete.location_class(),
        complete.location_function(),
        complete.location_size(),
    ] {
        let reference = reference.value().ok_or("reference")?;
        assert!(
            reference.resolved_index().is_some(),
            "compatible physical target"
        );
        assert!(
            reference.record_path().value().is_some(),
            "typed target path"
        );
        assert!(
            reference.target_type().value().is_some(),
            "typed target type"
        );
    }
    let sparse = sparse
        .classification()
        .value()
        .ok_or("sparse classification")?;
    assert_eq!(
        sparse.location_class().state(),
        SourceValueState::Null,
        "original null selector"
    );
    assert!(
        sparse
            .location_size()
            .value()
            .ok_or("dangling size")?
            .resolved_index()
            .is_none(),
        "dangling has no compatible target"
    );
    Ok(())
}

pub(super) fn tags(snapshot: &MissionLocationTemplateSnapshot<'_>) -> TestResult {
    let row = snapshot
        .rows
        .iter()
        .find(|row| row.names().value().is_some_and(|names| !names.is_empty()))
        .ok_or("template with original names")?;
    let names = row.names().value().ok_or("original name collection")?;
    assert_eq!(names.len(), 3, "name occurrences preserved");
    let tags = row.tags().value().ok_or("general tags")?;
    let dangling = tags
        .last()
        .and_then(|tag| tag.dangling_guid_source())
        .ok_or("dangling GUID source")?;
    assert!(
        dangling.value().is_some(),
        "full original GUID cell remains accessible"
    );
    assert!(
        !dangling.origin().source_uses_refs().is_empty(),
        "dangling operand has original evidence"
    );
    let reference = names
        .first()
        .and_then(SourcedValue::value)
        .ok_or("original first name variant")?
        .display_name
        .value()
        .ok_or("protected name reference")?;
    assert!(
        snapshot.game_text(reference).is_some(),
        "the referenced protected text is in the same snapshot"
    );
    Ok(())
}

fn first_spawn<'a, 'source>(
    snapshot: &'a MissionLocationTemplateSnapshot<'source>,
) -> TestResult<&'a p4kx_export::catalog_queries::LocationAutoSpawnSettings<'source>> {
    let config = snapshot
        .rows
        .first()
        .and_then(|row| row.configuration().value())
        .ok_or("configuration")?;
    config
        .auto_spawn_settings()
        .value()
        .and_then(|rows| rows.first())
        .and_then(|row| row.value())
        .ok_or_else(|| "first spawn setting".into())
}

pub(super) fn entity_cluster(snapshot: &MissionLocationTemplateSnapshot<'_>) -> TestResult {
    let row = snapshot
        .rows
        .first()
        .ok_or("template with a cluster selector")?;
    let reference = row
        .entity_cluster_member()
        .value()
        .ok_or("original cluster reference")?;
    let guid = reference.guid();
    let guid = guid.value().ok_or("original cluster target GUID")?;
    assert_eq!(
        guid.to_string(),
        crate::commodity_v3_fixture::guid(super::fixture::CLUSTER),
        "the nullable getter preserves the exact authored reference"
    );
    Ok(())
}

fn complete_spawn(snapshot: &MissionLocationTemplateSnapshot<'_>) -> TestResult {
    spawn(snapshot)?;
    let spawn = first_spawn(snapshot)?;
    assert_eq!(
        spawn.name().value().copied(),
        Some("FixtureSpawnName"),
        "original name"
    );
    assert_eq!(
        spawn.initial_activity().value().copied(),
        Some("FixtureSpawnActivity"),
        "original activity"
    );
    assert_eq!(
        spawn.exclude_ship_crew().value(),
        Some(&true),
        "authored crew flag"
    );
    assert_eq!(
        spawn.limits().max_group_size().value(),
        Some(&5),
        "group limit"
    );
    assert_eq!(
        spawn.limits().max_concurrent_spawns().value(),
        Some(&7),
        "concurrency limit"
    );
    assert_eq!(
        spawn.limits().max_spawns().value(),
        Some(&11),
        "total limit"
    );
    assert_eq!(
        spawn.mission_allied_marker().value(),
        Some(&false),
        "authored marker flag"
    );
    assert_eq!(
        spawn.is_critical().value(),
        Some(&true),
        "authored critical flag"
    );
    assert_eq!(
        spawn.authored_fields().roots().len(),
        1,
        "additional settings field stays typed"
    );
    let faction = spawn.faction_override().value().ok_or("faction selector")?;
    assert!(
        faction.guid().value().is_some(),
        "original faction selector remains present"
    );
    assert!(
        faction.resolved_index().is_none(),
        "fixture target remains unresolved"
    );
    Ok(())
}
