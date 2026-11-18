// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession};
use p4kx_export::mining_locations::{
    CompositionPart, DefinitionStatus, HarvestableElement, HarvestableOccurrenceRoute,
    MineableComposition, MineableElement, MiningLocationOptions, MiningLocationSnapshot,
    ResourceNumber,
};
use p4kx_extraction::{Archive, Guid, Language, SourceInventory, SourcedValue};

use crate::commodity_v3_fixture::{TestResult, guid};
use crate::harvest_fixture as fixture;

use super::origins;

#[test]
fn consumer_mining_joins_original_rows_and_refuses_unresolved_membership() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        fixture::write_archive(folder.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let session = CatalogQuerySession::open(
        &archive,
        CatalogLanguageSelection::Selected(&[Language::English]),
        None,
    )?;
    let snapshot = session
        .parsed(None)?
        .read_mining_locations(MiningLocationOptions::new(Language::English, None))?;
    let composition = check_join(&snapshot)?;
    let material = check_members(&snapshot, composition)?;
    check_locations_and_name(&snapshot, composition)?;
    assert!(
        snapshot.coverage.language_available,
        "the requested language was read"
    );
    assert_eq!(
        snapshot.coverage.unreadable_record_bodies, 0,
        "all selected bodies are readable"
    );
    let mut published = vec![material];
    let previous = published.clone();
    let result = publish_resolved_members(&snapshot, &mut published);
    assert_eq!(
        result
            .expect_err("a complete-resolved consumer must refuse")
            .to_string(),
        "consumer requires every composition member target",
        "the consumer policy keeps known dangling membership explicit"
    );
    assert_eq!(
        published, previous,
        "a refused refresh retains the owned cache"
    );
    Ok(())
}

fn check_join(snapshot: &MiningLocationSnapshot) -> TestResult<&MineableComposition> {
    let expected: Guid = guid(fixture::PRESET_PLACED).parse()?;
    let provider = snapshot
        .harvestable_provider_presets
        .iter()
        .find(|row| row.source.guid.value() == Some(&expected))
        .ok_or("placed provider")?;
    let [group] = provider.groups.value().ok_or("original groups")?.as_slice() else {
        return Err("one original provider group".into());
    };
    let [preset, direct] = group
        .value()
        .ok_or("group slot")?
        .harvestables
        .value()
        .ok_or("harvestable members")?
        .as_slice()
    else {
        return Err("two original entity-selection alternatives".into());
    };
    let expected: Guid = guid(fixture::COMPOSITION).parse()?;
    let composition = snapshot
        .mineable_compositions
        .iter()
        .find(|row| row.source.guid.value() == Some(&expected))
        .ok_or("composition")?;
    for member in [preset, direct] {
        check_spawn(snapshot, member, composition)?;
    }
    let preset_link = preset
        .value()
        .ok_or("preset member")?
        .harvestable
        .value()
        .ok_or("original HarvestablePreset selector")?;
    let expected: Guid = guid(fixture::HARVESTABLE).parse()?;
    assert_eq!(
        preset_link.guid().value(),
        Some(&expected),
        "the original preset is retained"
    );
    assert!(
        direct
            .value()
            .ok_or("direct member")?
            .harvestable
            .value()
            .is_none(),
        "the direct alternative has no fabricated preset"
    );
    closed(&provider.groups, &snapshot.source_inventory)?;
    Ok(composition)
}

fn check_spawn(
    snapshot: &MiningLocationSnapshot,
    member: &SourcedValue<HarvestableElement>,
    composition: &MineableComposition,
) -> TestResult {
    let entity = member
        .value()
        .ok_or("harvestable slot")?
        .spawn
        .entity
        .value()
        .ok_or("selected entity")?;
    let expected: Guid = guid(fixture::ENTITY).parse()?;
    assert_eq!(
        entity.guid.value(),
        Some(&expected),
        "same physical mineable entity"
    );
    let params = entity.mining_params.value().ok_or("mining component")?;
    let link = params.composition.value().ok_or("composition selector")?;
    let target = composition
        .source
        .record_index
        .value()
        .ok_or("physical composition Record")?;
    assert_eq!(
        link.target_record_index().value(),
        Some(target),
        "the actual selected composition joins by physical Record index"
    );
    closed(&params.composition, &snapshot.source_inventory)?;
    Ok(())
}

fn check_members(
    snapshot: &MiningLocationSnapshot,
    composition: &MineableComposition,
) -> TestResult<Guid> {
    let [first, second] = composition
        .parts
        .value()
        .ok_or("original parts")?
        .as_slice()
    else {
        return Err("two ordered original composition slots".into());
    };
    let first = first.value().ok_or("first part")?;
    let valid = first
        .mineable_element
        .value()
        .ok_or("original element reference")?;
    let index = valid
        .target_record_index()
        .value()
        .ok_or("physical element target")?;
    let element = snapshot
        .mineable_elements
        .iter()
        .find(|row| row.source.record_index.value() == Some(index))
        .ok_or("returned element")?;
    assert_eq!(
        valid.guid().value(),
        element.source.guid.value(),
        "element identity agrees"
    );
    check_resource(snapshot, element)?;
    assert!(
        matches!(first.min_percentage.value(), Some(ResourceNumber::Single(value))
        if value.raw_bits() == 30.0_f32.to_bits()),
        "the original percentage is not rescaled"
    );
    assert_eq!(
        first.min_percentage.origin().contains_encrypted_source(),
        Some(true),
        "the authored share uses the encrypted DataForge"
    );
    closed(&first.min_percentage, &snapshot.source_inventory)?;
    closed(&first.mineable_element, &snapshot.source_inventory)?;
    check_dangling(snapshot, second)?;
    valid
        .guid()
        .value()
        .copied()
        .ok_or_else(|| "original material GUID".into())
}

fn check_resource(snapshot: &MiningLocationSnapshot, element: &MineableElement) -> TestResult {
    let resource = element
        .resource_type
        .value()
        .ok_or("original ResourceType reference")?;
    let resource_index = resource
        .target_record_index()
        .value()
        .ok_or("physical resource target")?;
    let target = snapshot
        .resource_types
        .iter()
        .find(|row| row.source.record_index.value() == Some(resource_index))
        .ok_or("the returned physical resource")?;
    closed(&element.resource_type, &snapshot.source_inventory)?;
    closed(&target.source.guid, &snapshot.source_inventory)
}

fn check_dangling(
    snapshot: &MiningLocationSnapshot,
    member: &SourcedValue<CompositionPart>,
) -> TestResult {
    let dangling = member
        .value()
        .ok_or("second part")?
        .mineable_element
        .value()
        .ok_or("retained unresolved selector")?;
    assert!(
        matches!(dangling.status(), DefinitionStatus::Dangling),
        "known missing target"
    );
    assert!(
        dangling.target_record_index().value().is_none(),
        "no invented physical target"
    );
    closed(&dangling.guid(), &snapshot.source_inventory)
}

fn check_locations_and_name(
    snapshot: &MiningLocationSnapshot,
    composition: &MineableComposition,
) -> TestResult {
    let expected: Guid = guid(fixture::PRESET_PLACED).parse()?;
    let location = snapshot
        .harvestable_locations
        .iter()
        .find(|row| row.preset_guid == expected)
        .ok_or("placed provider location")?;
    let parent = location
        .starmap_object_guid
        .ok_or("the resolved Starmap parent")?;
    let object = snapshot
        .starmap_objects
        .iter()
        .find(|row| row.source.guid.value().copied() == Some(parent))
        .ok_or("resolved non-claim Starmap parent")?;
    closed(&object.position, &snapshot.source_inventory)?;
    let occurrence = snapshot
        .harvestable_occurrences
        .iter()
        .find(|row| row.preset_guid.value() == Some(&expected))
        .ok_or("physical occurrence")?;
    assert_eq!(
        occurrence.container_path.value().map(String::as_str),
        Some(location.container.as_str()),
        "physical occurrence joins the logical location"
    );
    assert_eq!(
        occurrence.starmap_object_guid,
        Some(parent),
        "physical and logical Starmap parents agree"
    );
    let HarvestableOccurrenceRoute::Reached(route) = &occurrence.route else {
        return Err("the placed provider must have a reached physical route".into());
    };
    closed(&route.id, &snapshot.source_inventory)?;
    closed(&occurrence.preset_guid, &snapshot.source_inventory)?;
    let id = composition
        .deposit_name
        .value()
        .ok_or("composition text reference")?;
    let text = snapshot
        .game_texts
        .iter()
        .find(|row| row.id.value() == Some(id))
        .ok_or("returned protected text")?;
    let english = text
        .translations
        .value()
        .and_then(|rows| rows.get(Language::English.code()))
        .ok_or("selected locale forms")?;
    assert_eq!(
        english.display_text.value().map(String::as_str),
        Some("Iron Deposit"),
        "the consumed deposit label resolves through the selected locale"
    );
    closed(&english.display_text, &snapshot.source_inventory)
}

fn publish_resolved_members(
    snapshot: &MiningLocationSnapshot,
    published: &mut Vec<Guid>,
) -> TestResult {
    let mut pending = Vec::new();
    for composition in &snapshot.mineable_compositions {
        for slot in composition
            .parts
            .value()
            .ok_or("unavailable composition members")?
        {
            let reference = slot
                .value()
                .ok_or("unreadable composition slot")?
                .mineable_element
                .value()
                .ok_or("unavailable element selector")?;
            if !matches!(reference.status(), DefinitionStatus::Present) {
                return Err("consumer requires every composition member target".into());
            }
            pending.push(
                reference
                    .guid()
                    .value()
                    .copied()
                    .ok_or("original element GUID")?,
            );
        }
    }
    *published = pending;
    Ok(())
}

fn closed<T>(cell: &SourcedValue<T>, inventory: &SourceInventory) -> TestResult {
    origins::check(cell, inventory).map(|_| ())
}
