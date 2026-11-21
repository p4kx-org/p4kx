// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Guid;

use super::{NoObject, Objects, losses};
use crate::contract_harvest::HarvestableLocation;
use crate::guid_text::CatalogGuid;

fn guid(number: u8) -> Guid {
    format!("00000000-0000-0000-0000-0000000000{number:02x}")
        .parse()
        .expect("a GUID")
}

fn objects(entries: &[(u8, bool, Option<u8>)]) -> Objects {
    Objects {
        by_guid: entries
            .iter()
            .map(|&(own, claim, above)| (guid(own), (claim, above.map(guid))))
            .collect(),
    }
}

#[test]
fn a_plain_object_is_the_location() {
    let objects = objects(&[(1, false, None)]);
    assert_eq!(objects.location(Some(guid(1))), Ok(CatalogGuid(guid(1))));
}

#[test]
fn a_marker_gives_way_to_the_object_above() {
    let objects = objects(&[(1, false, None), (2, true, Some(1))]);
    assert_eq!(objects.location(Some(guid(2))), Ok(CatalogGuid(guid(1))));
}

#[test]
fn each_missing_object_says_why() {
    let objects = objects(&[(2, true, None)]);
    assert_eq!(objects.location(None), Err(NoObject::NoneAbove));
    assert_eq!(objects.location(Some(guid(9))), Err(NoObject::NotInCatalog));
    assert_eq!(
        objects.location(Some(guid(2))),
        Err(NoObject::MarkerWithoutParent)
    );
}

/// `count` markers, each hanging below the next, then a plain object.
fn markers_then_plain(count: u8) -> Objects {
    let entries: Vec<(u8, bool, Option<u8>)> = (1..=count)
        .map(|number| (number, true, Some(number + 1)))
        .chain([(count + 1, false, None)])
        .collect();
    objects(&entries)
}

#[test]
fn sixty_three_markers_still_reach_the_object_above() {
    let objects = markers_then_plain(63);
    assert_eq!(objects.location(Some(guid(1))), Ok(CatalogGuid(guid(64))));
}

#[test]
fn sixty_four_markers_reach_the_object_above() {
    let objects = markers_then_plain(64);
    assert_eq!(
        objects.location(Some(guid(1))),
        Ok(CatalogGuid(guid(65))),
        "all sixty-four original markers reach their terminal object"
    );
    assert_eq!(objects.location(Some(guid(2))), Ok(CatalogGuid(guid(65))));
}

#[test]
fn a_container_with_no_object_above_is_no_loss() {
    assert!(!NoObject::NoneAbove.is_loss());
    assert!(NoObject::NotInCatalog.is_loss());
}

#[test]
fn the_losses_count_every_reason_but_none_above() {
    let row = |reason| {
        (
            HarvestableLocation {
                preset_guid: CatalogGuid(guid(1)),
                starmap_object_guid: None,
                system: String::new(),
                container: String::new(),
            },
            reason,
        )
    };
    let rows = [
        row(None),
        row(Some(NoObject::NoneAbove)),
        row(Some(NoObject::NoneAbove)),
        row(Some(NoObject::NotInCatalog)),
        row(Some(NoObject::NotInCatalog)),
        row(Some(NoObject::MarkerWithoutParent)),
    ];
    assert_eq!(
        losses(&rows),
        vec![
            (NoObject::NotInCatalog, 2),
            (NoObject::MarkerWithoutParent, 1)
        ]
    );
}
