// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::Guid;

use super::Wanted;
use super::resolution::resolve_ordered;
use crate::LoadoutSummary;
use crate::contract_loadout::{EntityKind, Loadout, LoadoutPort};
use crate::guid_text::CatalogGuid;
use crate::loadout::staging::StagedLoadoutWriter;

fn guid(value: u128) -> CatalogGuid {
    CatalogGuid(Guid::from_stored(value.to_le_bytes()))
}

fn wanted(index: u32, guid: CatalogGuid, kind: EntityKind) -> Wanted {
    Wanted {
        record_index: index,
        guid,
        kind,
    }
}

fn resolved(want: Wanted) -> Loadout {
    Loadout {
        root_record_index: want.record_index,
        port_inventory_complete: true,
        entity_guid: want.guid,
        entity_kind: want.kind,
        issues: Vec::new(),
        ports: vec![LoadoutPort {
            name: want.record_index.to_string(),
            item_guid: Some(guid(99)),
            ..LoadoutPort::default()
        }],
    }
}

#[test]
fn a_guid_tie_preserves_wire_order_and_ship_ordinals() -> Result<(), Box<dyn std::error::Error>> {
    let scratch = tempfile::tempdir()?;
    let same = guid(2);
    let known = HashSet::new();
    let mut outside = HashSet::new();
    let mut summary = LoadoutSummary::default();
    let mut staged = StagedLoadoutWriter::new(scratch.path(), None);
    let wanted = [
        wanted(5, guid(3), EntityKind::Ship),
        wanted(1, guid(1), EntityKind::Item),
        wanted(2, same, EntityKind::Item),
        wanted(3, same, EntityKind::Ship),
        wanted(4, same, EntityKind::Item),
    ];

    let failures = resolve_ordered(
        wanted.into_iter(),
        None,
        |want| Ok((want.record_index != 4).then(|| resolved(want))),
        |entry| {
            summary.add_entry(&entry, &known, &mut outside);
            staged.append(entry)?;
            Ok(())
        },
    )?;
    assert!(failures.is_empty(), "the skipped row is not a failed read");
    let staged = staged.finish()?;
    let expected = [wanted[1], wanted[2], wanted[3], wanted[0]].map(resolved);
    assert_eq!(
        serde_json::to_vec(&staged)?,
        serde_json::to_vec(&expected)?,
        "canonical stable order includes emitted Item rows",
    );
    assert_eq!(
        staged
            .ships()
            .iter()
            .map(|ship| ship.ordinal)
            .collect::<Vec<_>>(),
        [2, 3],
        "ship ordinals include emitted Item rows and exclude skipped rows",
    );
    assert_eq!(summary.items, 2, "both emitted Item rows are counted");
    assert_eq!(summary.ships, 2, "both Ship rows are counted");
    assert_eq!(summary.attached, 4, "each occurrence contributes");
    assert_eq!(outside.len(), 1, "one outside GUID is shared by all rows");
    Ok(())
}
