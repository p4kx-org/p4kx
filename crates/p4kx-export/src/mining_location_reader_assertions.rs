// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, Language, SourcedValue};

use super::{PLANET, TestResult};
use crate::mining_locations::{
    ContainerInstanceKey, HarvestableOccurrenceRoute, MiningLocationSnapshot,
    ReachedHarvestableOccurrence,
};

fn check_composition(result: &MiningLocationSnapshot) -> TestResult {
    let element = result.mineable_elements.first().ok_or("element")?;
    let composition = result.mineable_compositions.first().ok_or("composition")?;
    let slot = composition
        .parts
        .value()
        .and_then(|parts| parts.first())
        .ok_or("authored slot")?;
    let part = slot.value().ok_or("authored share")?;
    let link = part.mineable_element.value().ok_or("element reference")?;
    assert_eq!(
        link.guid().value(),
        element.source.guid.value(),
        "the composition preserves its original reference"
    );
    assert!(
        matches!(
            part.min_percentage.value(),
            Some(crate::resource_number::ResourceNumber::Single(value))
                if value.raw_bits() == 20.0_f32.to_bits()
        ),
        "the authored share keeps its value and type"
    );
    assert!(
        closed(&composition.parts, result)
            && closed(slot, result)
            && closed(&part.min_percentage, result),
        "array, slot and field source use references resolve"
    );
    assert_eq!(
        part.min_percentage.origin().contains_encrypted_source(),
        Some(false),
        "the SDK proves clear Game2.dcb provenance"
    );
    assert!(
        closed(&element.source.guid, result),
        "header source use references resolve"
    );
    Ok(())
}

pub(super) fn check_links(result: &MiningLocationSnapshot) -> TestResult {
    check_composition(result)?;
    let provider = result
        .harvestable_provider_presets
        .first()
        .ok_or("provider")?;
    let planet = result.starmap_objects.first().ok_or("planet")?;
    let planet_guid: Guid = PLANET.parse()?;
    assert!(
        closed(&planet.position, result),
        "position source use references resolve"
    );
    for row in &result.harvestable_locations {
        assert_eq!(
            Some(&row.preset_guid),
            provider.source.guid.value(),
            "each location links to the admitted provider"
        );
        if row.container.ends_with("tile.socpak") {
            assert_eq!(
                row.starmap_object_guid,
                Some(planet_guid),
                "nearest Starmap parent"
            );
        } else {
            assert!(
                row.container.ends_with("unreached.socpak") && row.starmap_object_guid.is_none(),
                "the unreached location has no authored parent"
            );
        }
    }
    Ok(())
}

pub(super) fn check_occurrences(result: &MiningLocationSnapshot) -> TestResult {
    let mut reached = Vec::new();
    let mut unreached = 0;
    for row in &result.harvestable_occurrences {
        assert!(
            closed(&row.preset_guid, result),
            "verified SOC token uses resolve"
        );
        assert!(
            closed(&row.container, result),
            "the physical Entry has source evidence"
        );
        assert!(
            closed(&row.container_path, result),
            "the physical Entry path has source evidence"
        );
        assert!(
            result.harvestable_locations.iter().any(|logical| {
                Some(logical.container.as_str()) == row.container_path.value().map(String::as_str)
                    && logical.system == row.system
                    && Some(&logical.preset_guid) == row.preset_guid.value()
                    && logical.starmap_object_guid == row.starmap_object_guid
            }),
            "each physical row joins to the legacy logical location"
        );
        match &row.route {
            HarvestableOccurrenceRoute::Reached(route) => {
                reached.push((row, check_reached_route(route, result)?));
            }
            HarvestableOccurrenceRoute::Unreached => {
                assert!(
                    row.starmap_object_guid.is_none(),
                    "no inferred route or parent"
                );
                unreached += 1;
            }
        }
    }
    let [(first, first_id), (second, second_id)] = reached.as_slice() else {
        return Err("expected two reached routes to the same container".into());
    };
    assert_eq!(
        first.container.value(),
        second.container.value(),
        "one physical Entry"
    );
    assert_eq!(
        first.preset_guid.origin(),
        second.preset_guid.origin(),
        "both routes use the same verified physical token ranges"
    );
    assert_ne!(first_id, second_id, "separate authored Child routes");
    assert_eq!(
        unreached, 1,
        "the independent inventory-only Entry has no route"
    );
    Ok(())
}

fn check_reached_route<'route>(
    route: &'route ReachedHarvestableOccurrence,
    result: &MiningLocationSnapshot,
) -> Result<&'route ContainerInstanceKey, Box<dyn std::error::Error>> {
    for value in [&route.id, &route.parent] {
        assert!(closed(value, result), "route and parent selectors resolve");
    }
    assert!(
        closed(&route.root, result),
        "physical system root selectors resolve"
    );
    assert!(
        closed(&route.containing_record_guid, result),
        "original owner selector resolves"
    );
    let root = route.root.value().ok_or("root identity")?;
    let id = route.id.value().ok_or("physical occurrence ID")?;
    let parent = route.parent.value().ok_or("direct root parent")?;
    assert_eq!(
        id.root_entry_index(),
        root.entry_index(),
        "the route uses the exact root"
    );
    assert_eq!(
        parent.root_entry_index(),
        root.entry_index(),
        "the parent uses the same root"
    );
    assert!(
        parent.child_node_indices().is_empty(),
        "parent is the root container"
    );
    Ok(id)
}

pub(super) fn check_localization(result: &MiningLocationSnapshot) -> TestResult {
    let object = result.starmap_objects.first().ok_or("Starmap object")?;
    let name = object.name.value().ok_or("canonical name ID")?;
    let row = result
        .game_texts
        .iter()
        .find(|text| text.id.value() == Some(name))
        .ok_or("name resolves into protected selected text")?;
    let translations = row.translations.value().ok_or("text translations")?;
    assert_eq!(
        translations.len(),
        1,
        "only the requested locale is consulted"
    );
    let english = translations
        .get(Language::English.code())
        .ok_or("requested locale")?;
    assert_eq!(
        english.display_text.value().map(String::as_str),
        Some("Synthetic planet"),
        "legacy String name is resolved through the canonical Catalog policy"
    );
    assert!(
        closed(&english.display_text, result),
        "localized value's source uses resolve"
    );
    Ok(())
}

fn closed<T>(value: &SourcedValue<T>, result: &MiningLocationSnapshot) -> bool {
    let references = value.origin().source_uses_refs();
    !references.is_empty()
        && value.origin().source_set_complete()
        && references.iter().all(|reference| {
            result
                .source_inventory
                .uses()
                .iter()
                .any(|used| used.id() == reference)
        })
}
