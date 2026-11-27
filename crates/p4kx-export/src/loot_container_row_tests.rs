// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{Guid, SourceUses, SourceValue, SourceValueState};
use p4kx_extraction::{InputScope, SourceInput, SourceLocation, SourceUseRole};

use super::selection::first_placements;
use super::{PlacementValues, Tags, row, rows};
use crate::contract_loot::LootContainer;
use crate::guid_text::CatalogGuid;
use crate::harvest_locations::Objects;
use crate::loot_container_source::EntXmlAuthor;
use crate::loot_soc::EntityTags;
use crate::loot_unreadable::Unreadable;
use crate::loot_walk::{Placed, PlacedTarget, Summary};
use crate::starmap_walk::Opened;

#[path = "loot_container_author_tests.rs"]
mod author;

const CONFIG: &str = "687af3db-4c0d-4e8d-9261-102274f8b6ed";

fn placed(path: &str, sources: &SourceUses) -> Placed {
    Placed {
        author: EntXmlAuthor {
            containing_entry_index: 0,
            document_entry_index: 0,
        },
        config: CONFIG.parse::<Guid>().expect("a GUID"),
        config_origin: sources.origin(&[], false),
        path: path.to_owned(),
        path_origin: sources.origin(&[], false),
        target: PlacedTarget::Resolved(1),
    }
}

fn placement_summaries(first_config: Guid, sources: &SourceUses) -> HashMap<usize, Summary> {
    let alternate = Placed {
        target: PlacedTarget::Resolved(2),
        ..placed("a.socpak", sources)
    };
    HashMap::from([
        (
            10,
            Summary {
                placed: vec![placed("b.socpak", sources), placed("a.socpak", sources)],
                ..Summary::default()
            },
        ),
        (
            20,
            Summary {
                placed: vec![
                    alternate,
                    Placed {
                        config: first_config,
                        ..placed("z.socpak", sources)
                    },
                ],
                ..Summary::default()
            },
        ),
    ])
}

fn opened(entry: usize, system: &str) -> Opened {
    Opened {
        system: system.into(),
        system_entry: entry,
        entry,
        parent: crate::starmap_walk::OpenedParent::Unobserved,
        under: None,
    }
}

fn fixture_tags(
    first_config: Guid,
    second_config: Guid,
    sources: &SourceUses,
) -> Result<HashMap<usize, Tags>, crate::CatalogError> {
    let unbound = sources
        .observe(SourceInput::Unknown, InputScope::Payload)?
        .at(
            SourceLocation::Metadata {
                field: "fixtureTagId".to_owned(),
            },
            SourceUseRole::Payload,
        )?;
    Ok(HashMap::from([
        (
            1,
            Tags {
                tags: Ok(EntityTags {
                    entities: 3,
                    by_tag: HashMap::from([(first_config, 2)]),
                }),
                origin: sources.origin(&[], true),
                first_tag_uses: HashMap::from([(first_config, unbound.clone())]),
            },
        ),
        (
            2,
            Tags {
                tags: Ok(EntityTags {
                    entities: 5,
                    by_tag: HashMap::from([(second_config, 4)]),
                }),
                origin: sources.origin(&[], true),
                first_tag_uses: HashMap::from([(second_config, unbound)]),
            },
        ),
    ]))
}

#[test]
fn duplicate_rows_keep_the_first_placement_with_canonical_order()
-> Result<(), Box<dyn std::error::Error>> {
    let first_config: Guid = "67000000-0000-00ff-0000-000000000000".parse()?;
    let second_config: Guid = CONFIG.parse()?;
    let sources = &SourceUses::with_compact_use_refs();
    let summaries = placement_summaries(first_config, sources);
    let roots = [
        opened(10, "stanton"),
        opened(20, "stanton"),
        opened(20, "nyx"),
    ];
    let tags = fixture_tags(first_config, second_config, sources)?;
    let result = rows(
        first_placements(&roots, &Objects::new(&[]), &summaries),
        &tags,
        sources,
        |root, _| Ok(placement(sources, &root.system)),
    )?;
    let signatures: Vec<_> = result
        .iter()
        .map(|row| {
            (
                row.multi_config_guid.value().map(|guid| guid.0),
                row.container.value().map(String::as_str),
                system_name(row),
                row.entity_count.value().copied(),
            )
        })
        .collect();
    assert_eq!(
        signatures,
        [
            (Some(first_config), Some("z.socpak"), "nyx", Some(3)),
            (Some(first_config), Some("z.socpak"), "stanton", Some(3)),
            (Some(second_config), Some("a.socpak"), "nyx", Some(5)),
            (Some(second_config), Some("a.socpak"), "stanton", Some(3)),
            (Some(second_config), Some("b.socpak"), "stanton", Some(3)),
        ],
        "canonical row order and selected counts are stable"
    );
    let stanton = result
        .iter()
        .find(|row| {
            system_name(row) == "stanton"
                && row.container.value().map(String::as_str) == Some("a.socpak")
        })
        .ok_or("deduplicated Stanton placement")?;
    let Some([count]) = stanton.tag_counts.value().map(Vec::as_slice) else {
        return Err("expected one tag from the first placement".into());
    };
    assert_eq!(
        (count.tag_guid.value(), count.entities.value()),
        (Some(&CatalogGuid(first_config)), Some(&2)),
        "the first physical placement supplies the selected tag counts"
    );
    Ok(())
}

fn system_name(row: &LootContainer) -> &str {
    row.system.value().expect("a resolved system")
}

fn placement(sources: &SourceUses, system: &str) -> PlacementValues {
    PlacementValues {
        system: sources.value(SourceValue::Present(system.to_owned()), &[], false),
        object: sources.value(SourceValue::Absent, &[], false),
    }
}

#[test]
fn a_container_that_could_not_be_counted_has_no_entity_count() -> Result<(), crate::CatalogError> {
    let sources = &SourceUses::with_compact_use_refs();
    let failed = Tags {
        tags: Err(Unreadable::SocFile),
        origin: sources.origin(&[], false),
        first_tag_uses: HashMap::new(),
    };
    let row = row(
        &placed("a.socpak", sources),
        placement(sources, "stanton"),
        Some(&failed),
        sources,
    )?;
    assert_eq!(
        row.entity_count.state(),
        SourceValueState::Unreadable,
        "failed counts remain unavailable"
    );
    assert_eq!(
        row.tag_counts.state(),
        SourceValueState::Unreadable,
        "no partial tag counts survive"
    );
    Ok(())
}

#[test]
fn a_container_without_a_soc_has_zero_entities() -> Result<(), crate::CatalogError> {
    let sources = &SourceUses::with_compact_use_refs();
    let none = Tags {
        tags: Ok(EntityTags::default()),
        origin: sources.origin(&[], true),
        first_tag_uses: HashMap::new(),
    };
    let row = row(
        &placed("a.socpak", sources),
        placement(sources, "stanton"),
        Some(&none),
        sources,
    )?;
    assert_eq!(
        row.entity_count.value().copied(),
        Some(0),
        "an empty SOC selection has zero entities"
    );
    Ok(())
}

#[test]
fn a_container_the_archive_lacks_has_no_entity_count() -> Result<(), crate::CatalogError> {
    let sources = &SourceUses::with_compact_use_refs();
    let absent = Placed {
        target: PlacedTarget::VerifiedMissing,
        ..placed("a.socpak", sources)
    };
    let row = row(&absent, placement(sources, "stanton"), None, sources)?;
    assert_eq!(
        row.entity_count.state(),
        SourceValueState::Absent,
        "proved absence is distinct from an unreadable target"
    );
    Ok(())
}
