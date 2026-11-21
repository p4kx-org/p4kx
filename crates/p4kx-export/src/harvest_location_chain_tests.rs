// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Guid;

use super::{NoObject, Objects};
use crate::guid_text::CatalogGuid;

const MARKERS: u32 = 2_000;

fn guid(number: u32) -> Guid {
    Guid::from_stored(u128::from(number).to_le_bytes())
}

fn chain() -> Objects {
    Objects {
        by_guid: (1..=MARKERS)
            .map(|number| (guid(number), (true, Some(guid(number + 1)))))
            .collect(),
    }
}

#[test]
fn a_finite_marker_chain_reaches_its_actual_object() {
    let mut objects = chain();
    let terminal = guid(MARKERS + 1);
    objects.by_guid.insert(terminal, (false, None));
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
    objects.by_guid.insert(guid(MARKERS), (true, None));
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
                .map(|(own, parent)| (guid(own), (true, Some(guid(parent)))))
                .collect(),
        };
        assert_eq!(
            objects.location(Some(guid(1))).map_err(NoObject::reason),
            Err("mining claim markers form a cycle above the container"),
            "self-cycles, multi-marker cycles and an entering prefix are explicit"
        );
    }
}
