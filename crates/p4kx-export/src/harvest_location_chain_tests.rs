// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Guid, InputScope, SourceInput, SourceLocation, SourceUseRole, SourceUses, SourceValueState,
};

use super::{LocationObject, NoObject, Objects};
use crate::guid_text::CatalogGuid;

const MARKERS: u32 = 2_000;

fn guid(number: u32) -> Guid {
    Guid::from_stored(u128::from(number).to_le_bytes())
}

fn chain() -> Objects<'static> {
    Objects {
        by_guid: (1..=MARKERS)
            .map(|number| {
                (
                    guid(number),
                    LocationObject::unobserved(true, Some(guid(number + 1))),
                )
            })
            .collect(),
    }
}

#[test]
fn a_finite_marker_chain_reaches_its_actual_object() {
    let mut objects = chain();
    let terminal = guid(MARKERS + 1);
    objects
        .by_guid
        .insert(terminal, LocationObject::unobserved(false, None));
    assert_eq!(
        objects.location(Some(guid(1))),
        Ok(CatalogGuid(terminal)),
        "every finite parent link reaches the actual non-marker object"
    );
}

#[test]
fn long_chains_preserve_the_actual_missing_parent_reason() {
    let mut objects = chain();
    assert_eq!(
        objects.location(Some(guid(1))),
        Err(NoObject::NotInCatalog),
        "a missing terminal object remains a missing catalog object"
    );
    objects
        .by_guid
        .insert(guid(MARKERS), LocationObject::unobserved(true, None));
    assert_eq!(
        objects.location(Some(guid(1))),
        Err(NoObject::MarkerWithoutParent),
        "a finite terminal marker without a parent retains that distinct cause"
    );
}

#[test]
fn marker_cycles_have_an_explicit_cause() {
    for entries in [
        vec![(1, 1)],
        vec![(1, 2), (2, 1)],
        vec![(1, 2), (2, 3), (3, 2)],
    ] {
        let objects = Objects {
            by_guid: entries
                .into_iter()
                .map(|(own, parent)| {
                    (
                        guid(own),
                        LocationObject::unobserved(true, Some(guid(parent))),
                    )
                })
                .collect(),
        };
        assert_eq!(
            objects.location(Some(guid(1))).map_err(NoObject::reason),
            Err("mining claim markers form a cycle above the container"),
            "self-cycles, multi-marker cycles and an entering prefix are explicit"
        );
    }
}

#[test]
fn cyclical_origin_keeps_each_consulted_marker_selector() -> Result<(), crate::CatalogError> {
    let sources = SourceUses::with_compact_use_refs();
    let source = sources.observe(SourceInput::Unknown, InputScope::Payload)?;
    let mut uses = Vec::new();
    for step in 1..=3 {
        uses.push(source.at(
            SourceLocation::Metadata {
                field: format!("marker{step}"),
            },
            SourceUseRole::Selector,
        )?);
    }
    let origins: Vec<_> = uses
        .iter()
        .map(|used| sources.origin(std::slice::from_ref(used), true))
        .collect();
    let mut by_guid = std::collections::HashMap::new();
    for (index, (own, parent)) in [(1, 2), (2, 3), (3, 2)].into_iter().enumerate() {
        by_guid.insert(
            guid(own),
            LocationObject {
                origin: origins.get(index),
                ..LocationObject::unobserved(true, Some(guid(parent)))
            },
        );
    }
    let objects = Objects { by_guid };
    let result = objects.location_origin(Some(guid(1)), Err(NoObject::MarkerCycle), &sources)?;
    assert_eq!(
        result.state(),
        SourceValueState::Invalid,
        "the detected cycle has no usable GUID"
    );
    assert!(
        uses.iter().all(|used| result
            .origin()
            .source_uses_refs()
            .contains(used.reference())),
        "the proof includes every marker consulted by cycle detection"
    );
    assert!(
        !result.origin().source_set_complete(),
        "mock placement inputs do not prove actual parent edges"
    );
    Ok(())
}
