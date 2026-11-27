// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Guid, InputScope, SourceInput, SourceLocation, SourceUseRole, SourceUses, SourceValueState,
};

use super::{LocationObject, NoObject, Objects, losses};
use crate::guid_text::CatalogGuid;
use crate::mining_locations::HarvestableLocation;

fn guid(number: u8) -> Guid {
    format!("00000000-0000-0000-0000-0000000000{number:02x}")
        .parse()
        .expect("a GUID")
}

fn objects(entries: &[(u8, bool, Option<u8>)]) -> Objects<'static> {
    Objects::from_location_facts(entries.iter().map(|&(own, claim, above)| {
        (
            guid(own),
            LocationObject::unobserved(claim, above.map(guid)),
        )
    }))
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

#[test]
fn location_facts_preserve_the_last_alias_and_its_source_completeness()
-> Result<(), crate::CatalogError> {
    let sources = SourceUses::with_compact_use_refs();
    let input = sources.observe(SourceInput::Unknown, InputScope::Payload)?;
    let using = |field: &str| {
        input.at(
            SourceLocation::Metadata {
                field: field.to_owned(),
            },
            SourceUseRole::Selector,
        )
    };
    let early = using("earlierPhysicalDescription")?;
    let winning = using("winningPhysicalDescription")?;
    let early_origin = sources.origin(std::slice::from_ref(&early), true);
    let winning_origin = sources.origin(std::slice::from_ref(&winning), true);
    let selected = guid(1);
    let earlier = LocationObject::new(true, Some(guid(2)), Some(&early_origin), None);
    let winner = LocationObject::new(false, None, Some(&winning_origin), None);
    let actual = Ok(CatalogGuid(selected));

    let unique = Objects::from_location_facts([(selected, winner)]);
    assert!(
        unique
            .location_origin(Some(selected), actual, &sources)?
            .origin()
            .source_set_complete(),
        "one admitted physical source can prove the ancestor"
    );
    let aliases = Objects::from_location_facts([(selected, earlier), (selected, winner)]);
    assert_eq!(
        aliases.location(Some(selected)),
        actual,
        "the last physical alias wins"
    );
    let location = aliases.location_origin(Some(selected), actual, &sources)?;
    assert!(
        !location.origin().source_set_complete(),
        "the winner does not prove unique physical identity"
    );
    assert!(
        location
            .origin()
            .source_uses_refs()
            .contains(winning.reference())
            && !location
                .origin()
                .source_uses_refs()
                .contains(early.reference()),
        "the returned ancestor retains the actual winning description observation"
    );
    Ok(())
}

#[test]
fn location_fact_absence_distinguishes_a_root_from_an_unresolved_ancestor()
-> Result<(), crate::CatalogError> {
    let sources = SourceUses::with_compact_use_refs();
    let objects = Objects::from_location_facts([]);
    for (ancestor, outcome, complete) in [
        (None, NoObject::NoneAbove, true),
        (Some(guid(1)), NoObject::NotInCatalog, false),
    ] {
        let location = objects.location_origin(ancestor, Err(outcome), &sources)?;
        assert_eq!(
            location.state(),
            SourceValueState::Absent,
            "neither route has a value"
        );
        assert_eq!(
            location.origin().source_set_complete(),
            complete,
            "missing demanded input cannot certify the same absence as an actual root"
        );
    }
    Ok(())
}

/// `count` markers, each hanging below the next, then a plain object.
fn markers_then_plain(count: u8) -> Objects<'static> {
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
                preset_guid: guid(1),
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
