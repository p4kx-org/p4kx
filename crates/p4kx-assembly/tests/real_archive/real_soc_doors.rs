// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_assembly::{SocEntityIndex, SocEntityTarget};
use p4kx_extraction::cryxml::{SourceElement, SourceElementPosition};
use p4kx_extraction::{Archive, ArchiveWatch, Guid};
use p4kx_model::{SocEntity, SocEntityFile, SocInlineLoadoutKind, SocSourceValue};
use p4kx_testkit::{real_archive_key, real_archive_path};

use super::real_soc_rooms::{TestResult, bytes, registry_path, ship_containers};

#[derive(Default)]
struct Counts {
    links: usize,
    cross_stream: usize,
    sensor_to_door: usize,
    manual_entries: usize,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(
    clippy::print_stdout,
    reason = "only aggregate source coverage is reported"
)]
fn original_door_links_and_manual_overrides_match_one_source_container() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))
        .map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    for candidate in ship_containers(&archive) {
        watch
            .poll()
            .map_err(|_| "Archive changed during container selection")?;
        let Ok(container) = archive.open_container(candidate) else {
            continue;
        };
        if !container
            .entries()
            .iter()
            .any(|entry| registry_path(entry.path()))
        {
            continue;
        }
        let counts = compare(&container)?;
        if counts.sensor_to_door == 0 || counts.manual_entries == 0 {
            continue;
        }
        watch
            .check()
            .map_err(|_| "Archive changed during door verification")?;
        println!(
            "SOC doors: {} authored links, {} cross-stream targets, {} sensor-to-door links, {} manual entries",
            counts.links, counts.cross_stream, counts.sensor_to_door, counts.manual_entries
        );
        return Ok(());
    }
    Err("no source container with an explicit sensor-to-door link and manual entries".into())
}

struct Owner {
    stream: usize,
    chunk: usize,
    ordinal: usize,
    position: SourceElementPosition,
    door: bool,
}

fn compare(container: &Archive) -> TestResult<Counts> {
    let streams: Vec<_> = container
        .entries()
        .iter()
        .filter(|entry| entry.path().to_ascii_lowercase().ends_with(".soc"))
        .map(|entry| Ok((entry.index(), bytes(container, entry)?)))
        .collect::<TestResult<_>>()?;
    let mut index = SocEntityIndex::new();
    let mut originals = BTreeMap::<Guid, Vec<Owner>>::new();
    for (stream, raw) in &streams {
        for chunk in SocEntityFile::parse(raw)?.entity_chunks() {
            let chunk = chunk?;
            index.add_chunk(*stream, &chunk);
            for (ordinal, entity) in chunk
                .document()
                .root()
                .elements()
                .enumerate()
                .filter(|(_, node)| node.name() == "Entity")
            {
                let Some(guid) = entity.attribute("EntityCryGUID") else {
                    continue;
                };
                if let Ok(guid) = guid.parse::<Guid>() {
                    originals.entry(guid).or_default().push(Owner {
                        stream: *stream,
                        chunk: chunk.chunk_index(),
                        ordinal,
                        position: entity.position(),
                        door: has_property(entity, "SCItemDoor"),
                    });
                }
            }
        }
    }
    let mut counts = Counts::default();
    for (stream, raw) in &streams {
        for chunk in SocEntityFile::parse(raw)?.entity_chunks() {
            for entity in chunk?.entities() {
                compare_links(entity, *stream, &index, &originals, &mut counts)?;
                compare_loadouts(entity, &mut counts);
            }
        }
    }
    Ok(counts)
}

fn has_property(entity: SourceElement<'_>, name: &str) -> bool {
    entity
        .elements()
        .filter(|node| node.name() == "PropertiesDataCore")
        .flat_map(SourceElement::elements)
        .any(|node| node.name() == name)
}

fn verify_sensor(entity: SocEntity<'_>) -> bool {
    let sensor = entity
        .element()
        .elements()
        .filter(|node| node.name() == "PropertiesDataCore")
        .flat_map(SourceElement::elements)
        .any(|node| {
            node.name() == "SCItemProximitySensor"
                && node.attribute("__type") == Some("SCItemProximitySensorParams")
        });
    assert_eq!(
        entity
            .authored_components()
            .any(p4kx_model::SocEntityComponent::is_proximity_sensor),
        sensor,
        "typed sensor agrees with the original component scope"
    );
    sensor
}

fn compare_links(
    entity: SocEntity<'_>,
    stream: usize,
    index: &SocEntityIndex,
    originals: &BTreeMap<Guid, Vec<Owner>>,
    counts: &mut Counts,
) -> TestResult {
    let sensor = verify_sensor(entity);
    let raw_groups: Vec<_> = entity
        .element()
        .elements()
        .enumerate()
        .filter(|(_, group)| group.name() == "EntityLinks")
        .collect();
    let groups: Vec<_> = entity.entity_links().collect();
    assert_eq!(groups.len(), raw_groups.len(), "all original link groups");
    for (group, (ordinal, raw_group)) in groups.into_iter().zip(raw_groups) {
        assert_eq!(group.ordinal(), ordinal, "original group slot");
        let originals_in_group: Vec<_> = raw_group
            .elements()
            .enumerate()
            .filter(|(_, link)| link.name() == "Link")
            .collect();
        let links: Vec<_> = group.links().collect();
        assert_eq!(
            links.len(),
            originals_in_group.len(),
            "all original link occurrences"
        );
        for (link, (ordinal, raw)) in links.into_iter().zip(originals_in_group) {
            assert_eq!(link.ordinal(), ordinal, "original link slot");
            assert_eq!(
                link.element().position(),
                raw.position(),
                "original link coordinate"
            );
            let expected_name = raw
                .attribute("Name")
                .map_or(SocSourceValue::Absent, SocSourceValue::Present);
            assert!(link.name() == expected_name, "exact authored link name");
            let expected_target = raw.attribute("TargetCryGUID");
            assert!(
                link.target_attributes()
                    .map(|value| value.value())
                    .eq(expected_target),
                "exact authored target attribute"
            );
            let owner = compare_target(index.target(link), expected_target, originals)?;
            counts.links += 1;
            if let Some(owner) = owner {
                counts.cross_stream += usize::from(stream != owner.stream);
                counts.sensor_to_door += usize::from(sensor && owner.door);
            }
        }
    }
    Ok(())
}

fn compare_target<'owners>(
    observed: SocEntityTarget<'_>,
    target: Option<&str>,
    originals: &'owners BTreeMap<Guid, Vec<Owner>>,
) -> TestResult<Option<&'owners Owner>> {
    let Some(raw) = target else {
        return matches!(observed, SocEntityTarget::Absent)
            .then_some(None)
            .ok_or_else(|| "omitted link target differs".into());
    };
    let Ok(expected) = raw.parse::<Guid>() else {
        return matches!(observed, SocEntityTarget::Invalid)
            .then_some(None)
            .ok_or_else(|| "malformed link target differs".into());
    };
    let owners = originals.get(&expected).map_or(&[][..], Vec::as_slice);
    match (observed, owners) {
        (SocEntityTarget::Matched { guid, site }, [owner]) if guid == expected => {
            assert_eq!(
                (
                    site.stream_index,
                    site.chunk_index,
                    site.entity_ordinal,
                    &site.position
                ),
                (owner.stream, owner.chunk, owner.ordinal, &owner.position),
                "the original physical Entity owner"
            );
            Ok(Some(owner))
        }
        (SocEntityTarget::Unmatched { guid }, []) if guid == expected => Ok(None),
        (SocEntityTarget::Ambiguous { guid, sites }, owners)
            if guid == expected && owners.len() == sites.len() =>
        {
            Ok(None)
        }
        _ => Err("link resolution differs from original physical declarations".into()),
    }
}

fn compare_loadouts(entity: SocEntity<'_>, counts: &mut Counts) {
    let raw: Vec<_> = entity
        .element()
        .elements()
        .filter(|node| node.name() == "PropertiesDataCore")
        .flat_map(SourceElement::elements)
        .filter(|node| node.name() == "EntityComponentDefaultLoadout")
        .flat_map(SourceElement::elements)
        .filter(|node| node.name() == "loadout")
        .collect();
    let observed: Vec<_> = entity
        .authored_components()
        .flat_map(p4kx_model::SocEntityComponent::loadouts)
        .collect();
    assert_eq!(observed.len(), raw.len(), "all inline loadout declarations");
    for (loadout, raw) in observed.into_iter().zip(raw) {
        assert_eq!(
            loadout.element().position(),
            raw.position(),
            "original inline loadout coordinate"
        );
        if raw.attribute("__type") != Some("SItemPortLoadoutManualParams") {
            continue;
        }
        assert!(
            loadout.kind() == SocSourceValue::Present(SocInlineLoadoutKind::Manual),
            "original manual type"
        );
        compare_manual_entries(loadout, raw, counts);
    }
}

fn compare_manual_entries(
    loadout: p4kx_model::SocInlineLoadout<'_>,
    raw: SourceElement<'_>,
    counts: &mut Counts,
) {
    let original = raw
        .elements()
        .filter(|group| group.name() == "entries")
        .flat_map(SourceElement::elements)
        .filter(|entry| entry.name() == "SItemPortLoadoutEntryParams");
    let actual = loadout
        .entry_groups()
        .flat_map(p4kx_model::SocInlineLoadoutEntries::entries);
    let expected: Vec<_> = original.collect();
    let entries: Vec<_> = actual.collect();
    assert_eq!(entries.len(), expected.len(), "every authored manual entry");
    for (entry, raw) in entries.into_iter().zip(expected) {
        assert_eq!(
            entry.element().position(),
            raw.position(),
            "exact manual entry coordinate"
        );
        assert!(
            entry
                .class_reference_attributes()
                .map(|attr| attr.value())
                .eq(raw.attribute("entityClassReference")),
            "original class reference"
        );
        assert!(
            entry.port_name()
                == raw
                    .attribute("itemPortName")
                    .map_or(SocSourceValue::Absent, SocSourceValue::Present),
            "original port name"
        );
        counts.manual_entries += 1;
    }
}
