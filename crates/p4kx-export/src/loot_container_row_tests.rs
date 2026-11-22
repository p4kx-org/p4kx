// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{Guid, SourceUses};

use super::{Tags, row, rows};
use crate::guid_text::CatalogGuid;
use crate::harvest_locations::Objects;
use crate::loot_soc::EntityTags;
use crate::loot_unreadable::Unreadable;
use crate::loot_walk::{Placed, PlacedTarget, Summary};
use crate::starmap_walk::Opened;

const CONFIG: &str = "687af3db-4c0d-4e8d-9261-102274f8b6ed";

fn placed(path: &str, sources: &SourceUses) -> Placed {
    Placed {
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
        entry,
        under: None,
    }
}

fn fixture_tags(first_config: Guid, second_config: Guid) -> HashMap<usize, Tags> {
    HashMap::from([
        (
            1,
            Ok(EntityTags {
                entities: 3,
                by_tag: HashMap::from([(first_config, 2)]),
            }),
        ),
        (
            2,
            Ok(EntityTags {
                entities: 5,
                by_tag: HashMap::from([(second_config, 4)]),
            }),
        ),
    ])
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
    let tags = fixture_tags(first_config, second_config);
    let result = rows(&roots, &Objects::new(&[]), &summaries, &tags, sources)?;
    let signatures: Vec<_> = result
        .iter()
        .map(|row| {
            (
                row.multi_config_guid.value().map(|guid| guid.0),
                row.container.value().map(String::as_str),
                row.system.as_str(),
                row.entity_count,
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
            row.system == "stanton" && row.container.value().map(String::as_str) == Some("a.socpak")
        })
        .ok_or("deduplicated Stanton placement")?;
    let [count] = stanton.tag_counts.as_slice() else {
        return Err("expected one tag from the first placement".into());
    };
    assert_eq!(
        (count.tag_guid, count.entities),
        (CatalogGuid(first_config), 2),
        "the first physical placement supplies the selected tag counts"
    );
    Ok(())
}

#[test]
fn a_container_that_could_not_be_counted_has_no_entity_count() -> Result<(), crate::CatalogError> {
    let sources = &SourceUses::with_compact_use_refs();
    let failed: Tags = Err(Unreadable::SocFile);
    let row = row(
        &placed("a.socpak", sources),
        "stanton",
        None,
        Some(&failed),
        sources,
    )?;
    assert_eq!(row.entity_count, None, "failed counts remain unavailable");
    assert!(row.tag_counts.is_empty(), "no partial tag counts survive");
    Ok(())
}

#[test]
fn a_container_without_a_soc_has_zero_entities() -> Result<(), crate::CatalogError> {
    let sources = &SourceUses::with_compact_use_refs();
    let none: Tags = Ok(EntityTags::default());
    let row = row(
        &placed("a.socpak", sources),
        "stanton",
        None,
        Some(&none),
        sources,
    )?;
    assert_eq!(
        row.entity_count,
        Some(0),
        "an empty SOC selection has zero entities"
    );
    Ok(())
}

#[test]
fn a_container_the_archive_lacks_has_no_entity_count() -> Result<(), crate::CatalogError> {
    let sources = &SourceUses::with_compact_use_refs();
    let row = row(&placed("a.socpak", sources), "stanton", None, None, sources)?;
    assert_eq!(
        row.entity_count, None,
        "there are no counts without a physical target"
    );
    Ok(())
}
